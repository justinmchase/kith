use crate::ast::{Field, FlowStep, Instance, Next, Program, Statement, StepBody, Workflow};
use crate::codec::{decode, encode, json_u64};
use crate::dispatch::{ActorId, Dispatcher, Message};
use crate::error::Error;
use crate::host::{evaluate, match_field, match_message};
use serde_json::{json, Value as Json};
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    Inject,
    Down,
    Up,
    Join,
    Init,
    SelfSend,
}

struct ChildRef {
    id: ActorId,
    key: Option<String>,
    component: String,
}

struct JoinSet {
    remaining: HashSet<ActorId>,
    total: u64,
    completed: u64,
    reducer: Option<ActorId>,
}

#[derive(Clone)]
struct Timer {
    due: u64,
    to: ActorId,
    message: Json,
    route: Route,
    join: Option<u64>,
    slot: Option<usize>,
}

struct PendingJoin {
    remaining: usize,
    slots: Vec<Option<Json>>,
    to: ActorId,
    route: Route,
}

enum Slot {
    Ready(Json),
    Async { delay_ms: u64, value: Json },
}

enum Callee {
    Io,
    Async,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunStatus {
    Running,
    Succeeded,
    Failed,
}

struct WorkflowRun {
    status: RunStatus,
    step: String,
    input: Json,
    output: Json,
    definition: Workflow,
    prototypes: Vec<Instance>,
}

struct InstanceState {
    kind: String,
    parent: Option<ActorId>,
    children: Vec<ChildRef>,
    fields: HashMap<String, Json>,
    state: String,
    completed: bool,
    completion: Option<Json>,
    join: Option<JoinSet>,
    run: Option<WorkflowRun>,
}

pub struct World {
    program: Program,
    instances: HashMap<ActorId, InstanceState>,
    next_id: u64,
    dispatcher: Dispatcher,
    routes: HashMap<(ActorId, u64), Route>,
    route_seq: u64,
    prints: Vec<Json>,
    now: u64,
    rng: u64,
    timers: Vec<Timer>,
    joins: HashMap<u64, PendingJoin>,
    next_join: u64,
}

impl World {
    pub fn check(program: &Program) -> Result<(), Error> {
        if program.kind != "program" {
            return Err(Error::new("expected a program tree"));
        }
        let scene = program.unique_exported_scene().map_err(Error::new)?;
        if program.kind(&scene.root.component).is_none() {
            return Err(Error::new(format!("unknown kind {}", scene.root.component)));
        }
        for component in &program.components {
            if component.start_state().is_none() {
                return Err(Error::new(format!(
                    "component {} must declare exactly one start state",
                    component.name
                )));
            }
            check_ports(program, &component.ports)?;
            check_gotos(component)?;
            check_terminals(component)?;
            check_routes(component)?;
            check_effects(component)?;
        }
        for workflow in &program.workflows {
            check_workflow(program, workflow)?;
        }
        check_instance(program, &scene.root)?;
        Ok(())
    }

    pub fn start(program: Program) -> Result<Self, Error> {
        Self::check(&program)?;
        let scene = program.unique_exported_scene().map_err(Error::new)?.clone();
        let mut world = Self {
            program,
            instances: HashMap::new(),
            next_id: 1,
            dispatcher: Dispatcher::new(),
            routes: HashMap::new(),
            route_seq: 0,
            prints: Vec::new(),
            now: 0,
            rng: 1,
            timers: Vec::new(),
            joins: HashMap::new(),
            next_join: 1,
        };
        world.spawn_instance(&scene.root, None)?;
        world.enqueue_inits()?;
        Ok(world)
    }

    pub fn root_id(&self) -> ActorId {
        ActorId(1)
    }

    pub fn field(&self, id: ActorId, name: &str) -> Option<&Json> {
        self.instances.get(&id)?.fields.get(name)
    }

    pub fn output(&self, id: ActorId) -> Option<&Json> {
        let instance = self.instances.get(&id)?;
        if let Some(run) = &instance.run {
            return Some(&run.output);
        }
        instance.completion.as_ref()
    }

    pub fn child(&self, id: ActorId, port: &str) -> Option<ActorId> {
        self.instances
            .get(&id)?
            .children
            .iter()
            .find(|child| child.key.as_deref() == Some(port))
            .map(|child| child.id)
    }

    pub fn children_of_kind(&self, id: ActorId, kind: &str) -> Vec<ActorId> {
        self.instances
            .get(&id)
            .map(|instance| {
                instance
                    .children
                    .iter()
                    .filter(|child| child.component == kind)
                    .map(|child| child.id)
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn kind(&self, id: ActorId) -> Option<&str> {
        self.instances
            .get(&id)
            .map(|instance| instance.kind.as_str())
    }

    pub fn state(&self, id: ActorId) -> Option<&str> {
        self.instances
            .get(&id)
            .map(|instance| instance.state.as_str())
    }

    pub fn completed(&self, id: ActorId) -> bool {
        self.instances
            .get(&id)
            .is_some_and(|instance| instance.completed)
    }

    pub fn completion(&self, id: ActorId) -> Option<&Json> {
        self.instances.get(&id)?.completion.as_ref()
    }

    pub fn progress(&self, id: ActorId) -> Option<Json> {
        let join = self.instances.get(&id)?.join.as_ref()?;
        Some(json!({
            "completed": join.completed,
            "total": join.total,
        }))
    }

    pub fn root_fields(&self) -> HashMap<String, Json> {
        if let Some(output) = self.output(self.root_id()) {
            if let Json::Object(map) = output {
                return map.clone().into_iter().collect();
            }
        }
        self.instances
            .get(&self.root_id())
            .map(|instance| instance.fields.clone())
            .unwrap_or_default()
    }

    pub fn result(&self) -> Json {
        if let Some(output) = self.output(self.root_id()) {
            return output.clone();
        }
        serde_json::to_value(self.root_fields()).unwrap_or(Json::Null)
    }

    pub fn prints(&self) -> &[Json] {
        &self.prints
    }

    pub fn send(&mut self, id: ActorId, message: Json) -> Result<(), Error> {
        self.deliver(id, message, Route::Down)
    }

    pub fn inject(&mut self, message: Json) -> Result<(), Error> {
        if self.instances[&self.root_id()].run.is_some() {
            let sends = self.start_run(self.root_id(), message)?;
            for (to, message, route) in sends {
                self.deliver(to, message, route)?;
            }
            return Ok(());
        }
        self.deliver(self.root_id(), message, Route::Inject)
    }

    pub fn run_until_idle(&mut self) -> Result<(), Error> {
        loop {
            let Some(turn) = self.dispatcher.turn() else {
                return Ok(());
            };
            let route = self.take_route(turn.actor);
            let sends = self.handle(turn.actor, &turn.message.value, route)?;
            for (to, message, route) in sends {
                self.deliver(to, message, route)?;
            }
        }
    }

    pub fn run(&mut self) -> Result<(), Error> {
        loop {
            if !self.fire_next_timer()? {
                return Ok(());
            }
        }
    }

    /// Drain mail, then deliver the next due timer. Returns false if none.
    pub fn fire_next_timer(&mut self) -> Result<bool, Error> {
        self.run_until_idle()?;
        let Some(due) = self.timers.iter().map(|timer| timer.due).min() else {
            return Ok(false);
        };
        self.now = due;
        let mut due_now = Vec::new();
        self.timers.retain(|timer| {
            if timer.due <= self.now {
                due_now.push(timer.clone());
                false
            } else {
                true
            }
        });
        for timer in due_now {
            if let (Some(join_id), Some(slot)) = (timer.join, timer.slot) {
                if let Some(message) = self.fill_join(join_id, slot, timer.message)? {
                    self.deliver(timer.to, message, timer.route)?;
                }
            } else {
                self.deliver(timer.to, timer.message, timer.route)?;
            }
        }
        Ok(true)
    }

    pub fn snapshot(&self) -> Result<String, Error> {
        encode(&self.snapshot_value())
    }

    fn snapshot_value(&self) -> Json {
        let instances: Vec<Json> = self
            .instances
            .iter()
            .map(|(ActorId(id), instance)| {
                json!({
                    "id": id,
                    "kind": instance.kind,
                    "parent": instance.parent.map(|ActorId(id)| id),
                    "children": instance.children.iter().map(|child| json!({
                        "id": child.id.0,
                        "key": child.key,
                        "component": child.component,
                    })).collect::<Vec<_>>(),
                    "fields": instance.fields,
                    "state": instance.state,
                    "completed": instance.completed,
                    "completion": instance.completion,
                    "join": instance.join.as_ref().map(|join| json!({
                        "remaining": join.remaining.iter().map(|ActorId(id)| *id).collect::<Vec<_>>(),
                        "total": join.total,
                        "completed": join.completed,
                        "reducer": join.reducer.map(|ActorId(id)| id),
                    })),
                    "run": instance.run.as_ref().map(|run| json!({
                        "status": match run.status {
                            RunStatus::Running => "running",
                            RunStatus::Succeeded => "succeeded",
                            RunStatus::Failed => "failed",
                        },
                        "step": run.step,
                        "input": run.input,
                        "output": run.output,
                        "definition": run.definition,
                        "prototypes": run.prototypes,
                    })),
                })
            })
            .collect();
        let routes: Vec<Json> = self
            .routes
            .iter()
            .map(|((ActorId(id), seq), route)| {
                json!({
                    "id": id,
                    "seq": seq,
                    "route": snapshot_route(*route)
                })
            })
            .collect();
        json!({
            "nextId": self.next_id,
            "routeSeq": self.route_seq,
            "now": self.now,
            "rng": self.rng,
            "instances": instances,
            "dispatcher": self.dispatcher.snapshot(),
            "routes": routes,
            "prints": self.prints,
            "nextJoin": self.next_join,
            "joins": self.joins.iter().map(|(id, join)| json!({
                "id": id,
                "remaining": join.remaining,
                "slots": join.slots,
                "to": join.to.0,
                "route": snapshot_route(join.route),
            })).collect::<Vec<_>>(),
            "timers": self.timers.iter().map(|timer| json!({
                "due": timer.due,
                "to": timer.to.0,
                "message": timer.message,
                "route": snapshot_route(timer.route),
                "join": timer.join,
                "slot": timer.slot,
            })).collect::<Vec<_>>(),
        })
    }

    pub fn restore(program: Program, snapshot: &str) -> Result<Self, Error> {
        Self::check(&program)?;
        let snapshot = decode(snapshot)?;
        let next_id = snapshot
            .get("nextId")
            .and_then(json_u64)
            .ok_or_else(|| Error::new("snapshot missing nextId"))?;
        let route_seq = snapshot.get("routeSeq").and_then(json_u64).unwrap_or(0);
        let dispatcher = Dispatcher::restore(
            snapshot
                .get("dispatcher")
                .ok_or_else(|| Error::new("snapshot missing dispatcher"))?,
        )
        .map_err(Error::new)?;
        let mut instances = HashMap::new();
        for entry in snapshot
            .get("instances")
            .and_then(Json::as_array)
            .ok_or_else(|| Error::new("snapshot missing instances"))?
        {
            let id = ActorId(
                entry
                    .get("id")
                    .and_then(json_u64)
                    .ok_or_else(|| Error::new("instance missing id"))?,
            );
            instances.insert(id, restore_instance(entry)?);
        }
        let mut routes = HashMap::new();
        if let Some(list) = snapshot.get("routes").and_then(Json::as_array) {
            for entry in list {
                let id = ActorId(
                    entry
                        .get("id")
                        .and_then(json_u64)
                        .ok_or_else(|| Error::new("route missing id"))?,
                );
                let seq = entry.get("seq").and_then(json_u64).unwrap_or(0);
                let route = restore_route(entry.get("route").and_then(Json::as_str));
                routes.insert((id, seq), route);
            }
        }
        let prints = snapshot
            .get("prints")
            .and_then(Json::as_array)
            .cloned()
            .unwrap_or_default();
        let now = snapshot.get("now").and_then(json_u64).unwrap_or(0);
        let rng = snapshot.get("rng").and_then(json_u64).unwrap_or(1);
        let next_join = snapshot.get("nextJoin").and_then(json_u64).unwrap_or(1);
        let mut timers = Vec::new();
        if let Some(list) = snapshot.get("timers").and_then(Json::as_array) {
            for entry in list {
                timers.push(restore_timer(entry)?);
            }
        }
        let mut joins = HashMap::new();
        if let Some(list) = snapshot.get("joins").and_then(Json::as_array) {
            for entry in list {
                let id = entry
                    .get("id")
                    .and_then(json_u64)
                    .ok_or_else(|| Error::new("join missing id"))?;
                joins.insert(id, restore_join(entry)?);
            }
        }
        Ok(Self {
            program,
            instances,
            next_id,
            dispatcher,
            routes,
            route_seq,
            prints,
            now,
            rng,
            timers,
            joins,
            next_join,
        })
    }

    fn spawn_instance(
        &mut self,
        node: &Instance,
        parent: Option<ActorId>,
    ) -> Result<ActorId, Error> {
        let id = self.spawn_kind(&node.component, parent)?;
        if self.instances[&id].run.is_some() {
            if let Some(run) = self
                .instances
                .get_mut(&id)
                .and_then(|instance| instance.run.as_mut())
            {
                run.prototypes = node.children.clone();
            }
            return Ok(id);
        }
        let mut children = Vec::new();
        for child in &node.children {
            let child_id = self.spawn_instance(child, Some(id))?;
            children.push(ChildRef {
                id: child_id,
                key: child.key.clone(),
                component: child.component.clone(),
            });
        }
        self.instances.get_mut(&id).expect("parent").children = children;
        Ok(id)
    }

    fn enqueue_inits(&mut self) -> Result<(), Error> {
        let mut ids: Vec<_> = self
            .instances
            .iter()
            .filter(|(_, instance)| instance.run.is_none())
            .map(|(id, _)| *id)
            .collect();
        ids.sort_by_key(|id| id.0);
        for id in ids {
            self.deliver(id, Json::Null, Route::Init)?;
        }
        Ok(())
    }

    fn spawn_kind(&mut self, kind: &str, parent: Option<ActorId>) -> Result<ActorId, Error> {
        if let Some(workflow) = self.program.workflow(kind).cloned() {
            return self.spawn_workflow(workflow, parent);
        }
        let component = self
            .program
            .component(kind)
            .ok_or_else(|| Error::new(format!("unknown kind {kind}")))?
            .clone();
        let start = component
            .start_state()
            .ok_or_else(|| Error::new(format!("kind {kind} has no start")))?
            .name
            .clone();
        let id = ActorId(self.next_id);
        self.next_id += 1;
        self.dispatcher.register(id);
        let mut fields = HashMap::new();
        for field in &component.fields {
            let value = if let Some(init) = &field.init {
                self.eval(init, &HashMap::new())?
            } else {
                Json::Null
            };
            self.match_declared_field(&component.fields, &field.name, &value)?;
            fields.insert(field.name.clone(), value);
        }
        self.instances.insert(
            id,
            InstanceState {
                kind: kind.to_string(),
                parent,
                children: Vec::new(),
                fields,
                state: start,
                completed: false,
                completion: None,
                join: None,
                run: None,
            },
        );
        Ok(id)
    }

    fn spawn_workflow(
        &mut self,
        workflow: Workflow,
        parent: Option<ActorId>,
    ) -> Result<ActorId, Error> {
        let start = workflow
            .start_step()
            .ok_or_else(|| Error::new(format!("workflow {} has no start", workflow.name)))?
            .name
            .clone();
        let id = ActorId(self.next_id);
        self.next_id += 1;
        self.dispatcher.register(id);
        self.instances.insert(
            id,
            InstanceState {
                kind: workflow.name.clone(),
                parent,
                children: Vec::new(),
                fields: HashMap::new(),
                state: start.clone(),
                completed: false,
                completion: None,
                join: None,
                run: Some(WorkflowRun {
                    status: RunStatus::Running,
                    step: start,
                    input: Json::Null,
                    output: Json::Null,
                    definition: workflow,
                    prototypes: Vec::new(),
                }),
            },
        );
        Ok(id)
    }

    fn start_run(
        &mut self,
        id: ActorId,
        input: Json,
    ) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        if let Some(run) = self
            .instances
            .get_mut(&id)
            .and_then(|instance| instance.run.as_mut())
        {
            run.input = input.clone();
            run.output = input;
            run.status = RunStatus::Running;
        }
        self.enter_current(id)
    }

    fn enter_current(&mut self, id: ActorId) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        let (step, output) = {
            let run = self.instances[&id]
                .run
                .as_ref()
                .ok_or_else(|| Error::new("not a workflow"))?;
            let step = run
                .definition
                .step(&run.step)
                .cloned()
                .ok_or_else(|| Error::new(format!("unknown step {}", run.step)))?;
            (step, run.output.clone())
        };
        self.enter_step(id, &step, output)
    }

    fn enter_step(
        &mut self,
        id: ActorId,
        step: &FlowStep,
        working: Json,
    ) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        if let Some(run) = self
            .instances
            .get_mut(&id)
            .and_then(|instance| instance.run.as_mut())
        {
            run.step = step.name.clone();
            run.output = working.clone();
        }
        let bindings = match &step.pattern {
            Some(pattern) => bind_step(pattern, &working)?,
            None => bindings_from(&working),
        };
        match &step.body {
            StepBody::Succeed {} => self.succeed(id, working),
            StepBody::Fail { error } => {
                let value = self.eval(error, &bindings)?;
                self.fail(id, value)
            }
            StepBody::Pass { value } => {
                let output = self.eval(value, &bindings)?;
                self.advance(id, output, step.next.as_ref())
            }
            StepBody::Choice { clauses } => {
                for clause in clauses {
                    if match_message(&clause.pattern, &working)?.is_some() {
                        return self.goto_named(id, &clause.step);
                    }
                }
                Err(Error::new(format!("no choice matched on {}", step.name)))
            }
            StepBody::Task { component } => self.spawn_task(id, component, working),
            StepBody::Map {
                items,
                component,
                reduce,
                init,
            } => self.spawn_map(id, items, component, reduce, init.as_ref(), &bindings),
            StepBody::Run { path } => {
                let plan = working
                    .get(path)
                    .cloned()
                    .ok_or_else(|| Error::new(format!("run path {path} missing")))?;
                let definition = workflow_from_value(plan)?;
                let child = self.spawn_workflow(definition, Some(id))?;
                let kind = self.instances[&child].kind.clone();
                self.instances
                    .get_mut(&id)
                    .expect("actor")
                    .children
                    .push(ChildRef {
                        id: child,
                        key: None,
                        component: kind,
                    });
                self.arm_join(id, [child].into_iter().collect(), 1, None);
                self.start_run(child, working)
            }
        }
    }

    fn spawn_task(
        &mut self,
        id: ActorId,
        component: &str,
        working: Json,
    ) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        let child = self.spawn_work(id, component)?;
        self.arm_join(id, [child].into_iter().collect(), 1, None);
        self.start_work(child, working)
    }

    fn spawn_map(
        &mut self,
        id: ActorId,
        items_expr: &Json,
        component: &str,
        reduce: &str,
        init: Option<&Json>,
        bindings: &HashMap<String, Json>,
    ) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        let reducer = self.spawn_work(id, reduce)?;
        if let Some(init) = init {
            let value = self.eval(init, bindings)?;
            self.apply_fields(reducer, value)?;
        }
        let items = self.eval(items_expr, bindings)?;
        let items = items
            .as_array()
            .ok_or_else(|| Error::new("fanout expects a list"))?
            .clone();
        let mut children = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let child = self.spawn_work(id, component)?;
            children.push((
                child,
                json!({
                    "kind": "Item",
                    "item": item,
                    "index": index as f64,
                }),
            ));
        }
        let remaining: HashSet<_> = children.iter().map(|(child, _)| *child).collect();
        let total = remaining.len() as u64;
        self.arm_join(id, remaining, total, Some(reducer));
        if children.is_empty() {
            return self.end_accumulator(reducer);
        }
        let mut sends = Vec::new();
        for (child, message) in children {
            sends.extend(self.start_work(child, message)?);
        }
        Ok(sends)
    }

    fn start_work(
        &mut self,
        child: ActorId,
        message: Json,
    ) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        if self.instances[&child].run.is_some() {
            self.start_run(child, message)
        } else {
            Ok(vec![(child, message, Route::Down)])
        }
    }

    fn end_accumulator(&mut self, child: ActorId) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        Ok(vec![(child, Json::Null, Route::Join)])
    }

    fn spawn_work(&mut self, parent: ActorId, kind: &str) -> Result<ActorId, Error> {
        let prototype = self.instances[&parent].run.as_ref().and_then(|run| {
            run.prototypes
                .iter()
                .find(|node| node.component == kind)
                .cloned()
        });
        if prototype.is_none() {
            if let Some(declared) = self.program.kind(kind) {
                if !declared.ports.is_empty() {
                    return Err(Error::new(format!(
                        "fanout kind {kind} must not declare ports"
                    )));
                }
            }
        }
        let child = if let Some(node) = prototype {
            self.spawn_instance(&node, Some(parent))?
        } else {
            self.spawn_kind(kind, Some(parent))?
        };
        self.instances
            .get_mut(&parent)
            .expect("actor")
            .children
            .push(ChildRef {
                id: child,
                key: None,
                component: kind.to_string(),
            });
        Ok(child)
    }

    fn match_declared_field(
        &self,
        fields: &[Field],
        name: &str,
        value: &Json,
    ) -> Result<(), Error> {
        let field = fields
            .iter()
            .find(|field| field.name == name)
            .ok_or_else(|| Error::new(format!("unknown field {name}")))?;
        let Some(pattern) = &field.pattern else {
            return Ok(());
        };
        if match_field(pattern, value)? {
            Ok(())
        } else {
            Err(Error::new(format!("field {name} did not match")))
        }
    }

    fn write_field(&mut self, id: ActorId, name: &str, value: Json) -> Result<(), Error> {
        let kind = self.instances[&id].kind.clone();
        let fields = self
            .program
            .component(&kind)
            .ok_or_else(|| Error::new("unknown kind"))?
            .fields
            .clone();
        self.match_declared_field(&fields, name, &value)?;
        self.instances
            .get_mut(&id)
            .expect("actor")
            .fields
            .insert(name.to_string(), value);
        Ok(())
    }

    fn apply_fields(&mut self, id: ActorId, value: Json) -> Result<(), Error> {
        let kind = self.instances[&id].kind.clone();
        let Json::Object(map) = value else {
            return Err(Error::new(format!(
                "into {kind} expected an object of fields"
            )));
        };
        for (name, field_value) in map {
            if !self.instances[&id].fields.contains_key(&name) {
                return Err(Error::new(format!("unknown field {name} on {kind}")));
            }
            self.write_field(id, &name, field_value)?;
        }
        Ok(())
    }

    fn arm_join(
        &mut self,
        id: ActorId,
        remaining: HashSet<ActorId>,
        total: u64,
        reducer: Option<ActorId>,
    ) {
        self.instances.get_mut(&id).expect("actor").join = Some(JoinSet {
            remaining,
            total,
            completed: 0,
            reducer,
        });
    }

    fn current_next(&self, id: ActorId) -> Option<Next> {
        let run = self.instances.get(&id)?.run.as_ref()?;
        run.definition.step(&run.step)?.next.clone()
    }

    fn goto_named(
        &mut self,
        id: ActorId,
        name: &str,
    ) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        let (step, output) = {
            let run = self.instances[&id]
                .run
                .as_ref()
                .ok_or_else(|| Error::new("not a workflow"))?;
            let step = run
                .definition
                .step(name)
                .cloned()
                .ok_or_else(|| Error::new(format!("unknown step {name}")))?;
            (step, run.output.clone())
        };
        self.enter_step(id, &step, output)
    }

    fn advance(
        &mut self,
        id: ActorId,
        output: Json,
        next: Option<&Next>,
    ) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        if let Some(run) = self
            .instances
            .get_mut(&id)
            .and_then(|instance| instance.run.as_mut())
        {
            run.output = output.clone();
        }
        match next {
            None | Some(Next::Succeed {}) => self.succeed(id, output),
            Some(Next::Fail { error }) => {
                let value = self.eval(error, &bindings_from(&output))?;
                self.fail(id, value)
            }
            Some(Next::Goto { step }) => self.goto_named(id, step),
        }
    }

    fn succeed(&mut self, id: ActorId, output: Json) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        {
            let instance = self.instances.get_mut(&id).expect("actor");
            instance.completed = true;
            instance.completion = Some(output.clone());
            if let Some(run) = instance.run.as_mut() {
                run.status = RunStatus::Succeeded;
                run.output = output.clone();
            }
        }
        self.finish_to_parent(id, output)
    }

    fn fail(&mut self, id: ActorId, error: Json) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        {
            let instance = self.instances.get_mut(&id).expect("actor");
            instance.completed = true;
            instance.completion = Some(error.clone());
            if let Some(run) = instance.run.as_mut() {
                run.status = RunStatus::Failed;
                run.output = error.clone();
            }
        }
        Err(Error::new(format!("workflow failed: {error}")))
    }

    fn finish_to_parent(
        &mut self,
        id: ActorId,
        output: Json,
    ) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        let Some(parent) = self.instances[&id].parent else {
            return Ok(vec![]);
        };
        if self.instances[&parent].run.is_some() {
            return self.on_work_complete(parent, id, output);
        }
        Ok(vec![(parent, output, Route::Up)])
    }

    fn handle(
        &mut self,
        id: ActorId,
        message: &Json,
        route: Route,
    ) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        if self.instances[&id].run.is_some() {
            return match route {
                Route::Join => {
                    let next = self.current_next(id);
                    self.instances.get_mut(&id).expect("actor").join = None;
                    self.advance(id, message.clone(), next.as_ref())
                }
                Route::Inject => self.start_run(id, message.clone()),
                Route::Up | Route::Init => Ok(vec![]),
                Route::Down => Err(Error::new("workflow does not take down messages")),
                Route::SelfSend => Err(Error::new("workflow does not take self messages")),
            };
        }
        let (kind_name, state_name, fields, completed, parent) = {
            let instance = self
                .instances
                .get(&id)
                .ok_or_else(|| Error::new("unknown actor"))?;
            (
                instance.kind.clone(),
                instance.state.clone(),
                instance.fields.clone(),
                instance.completed,
                instance.parent,
            )
        };
        if completed {
            return unmatched(route, &kind_name, parent, message);
        }
        let component = self
            .program
            .component(&kind_name)
            .ok_or_else(|| Error::new("unknown kind"))?
            .clone();
        let state = component
            .states
            .iter()
            .find(|state| state.name == state_name)
            .ok_or_else(|| Error::new("unknown state"))?;
        if route == Route::Join {
            let Some(handler) = state.handlers.iter().rev().find(|handler| {
                handler.item.is_none()
                    && handler.pattern.get("kind").and_then(Json::as_str) == Some("any")
            }) else {
                return unmatched(route, &kind_name, self.instances[&id].parent, message);
            };
            let mut bindings = HashMap::new();
            for (name, value) in &fields {
                bindings.insert(name.clone(), value.clone());
            }
            bind_world(&mut bindings);
            return self.run_statements(id, &handler.body, &bindings);
        }
        for handler in &state.handlers {
            if !route_matches(&handler.route, route) {
                continue;
            }
            if let Some(mut bindings) = match_handler(handler, message)? {
                for (name, value) in &fields {
                    bindings.entry(name.clone()).or_insert(value.clone());
                }
                bind_world(&mut bindings);
                return self.run_statements(id, &handler.body, &bindings);
            }
        }
        unmatched(route, &kind_name, self.instances[&id].parent, message)
    }

    fn run_statements(
        &mut self,
        id: ActorId,
        statements: &[Statement],
        bindings: &HashMap<String, Json>,
    ) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        let mut sends = Vec::new();
        let mut bindings = bindings.clone();
        for statement in statements {
            match statement {
                Statement::Assign { field, expression } => {
                    let value = self.eval(expression, &bindings)?;
                    self.write_field(id, field, value.clone())?;
                    bindings.insert(field.clone(), value);
                }
                Statement::SendUp { message, after } => {
                    let parent = self.instances[&id]
                        .parent
                        .ok_or_else(|| Error::new("send up from root"))?;
                    self.enqueue_send(
                        parent,
                        message,
                        after.as_ref(),
                        Route::Up,
                        &bindings,
                        &mut sends,
                    )?;
                }
                Statement::SendTo {
                    port,
                    message,
                    after,
                } => {
                    let child = self.instances[&id]
                        .children
                        .iter()
                        .find(|child| child.key.as_deref() == Some(port.as_str()))
                        .ok_or_else(|| Error::new(format!("unknown port {port}")))?;
                    self.enqueue_send(
                        child.id,
                        message,
                        after.as_ref(),
                        Route::Down,
                        &bindings,
                        &mut sends,
                    )?;
                }
                Statement::SendDown {
                    component,
                    message,
                    after,
                } => {
                    let selected = self.select_children(id, component.as_deref());
                    if selected.is_empty() {
                        return Err(Error::new("no child matched send down"));
                    }
                    for child_id in selected {
                        self.enqueue_send(
                            child_id,
                            message,
                            after.as_ref(),
                            Route::Down,
                            &bindings,
                            &mut sends,
                        )?;
                    }
                }
                Statement::Send { message, after } => {
                    self.enqueue_send(
                        id,
                        message,
                        after.as_ref(),
                        Route::SelfSend,
                        &bindings,
                        &mut sends,
                    )?;
                }
                Statement::Expr { expression } => {
                    let _ = self.eval(expression, &bindings)?;
                }
                Statement::Goto { state } => {
                    let kind = self.instances[&id].kind.clone();
                    let known = self.program.component(&kind).is_some_and(|component| {
                        component
                            .states
                            .iter()
                            .any(|declared| declared.named && declared.name == *state)
                    });
                    if !known {
                        return Err(Error::new(format!("unknown state {state}")));
                    }
                    self.instances.get_mut(&id).expect("actor").state = state.clone();
                    break;
                }
                Statement::Complete { message } => {
                    let value = self.eval(message, &bindings)?;
                    {
                        let instance = self.instances.get_mut(&id).expect("actor");
                        instance.completed = true;
                        instance.completion = Some(value.clone());
                    }
                    sends.extend(self.finish_to_parent(id, value)?);
                    break;
                }
            }
        }
        Ok(sends)
    }

    fn on_work_complete(
        &mut self,
        parent: ActorId,
        child: ActorId,
        payload: Json,
    ) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        let Some(join) = self
            .instances
            .get(&parent)
            .and_then(|instance| instance.join.as_ref())
        else {
            return Ok(vec![]);
        };
        if join.reducer == Some(child) {
            self.instances.get_mut(&parent).expect("actor").join = None;
            let next = self.current_next(parent);
            return self.advance(parent, payload, next.as_ref());
        }
        let reducer = join.reducer;
        let removed = self
            .instances
            .get_mut(&parent)
            .and_then(|instance| instance.join.as_mut())
            .is_some_and(|join| join.remaining.remove(&child));
        if !removed {
            return Ok(vec![]);
        }
        if let Some(join) = self
            .instances
            .get_mut(&parent)
            .and_then(|instance| instance.join.as_mut())
        {
            join.completed += 1;
        }
        if let Some(reducer) = reducer {
            let mut sends = self.start_work(reducer, payload)?;
            let empty = self
                .instances
                .get(&parent)
                .and_then(|instance| instance.join.as_ref())
                .is_some_and(|join| join.remaining.is_empty());
            if empty {
                sends.extend(self.end_accumulator(reducer)?);
            }
            return Ok(sends);
        }
        let done = self
            .instances
            .get(&parent)
            .and_then(|instance| instance.join.as_ref())
            .is_some_and(|join| join.remaining.is_empty());
        if !done {
            return Ok(vec![]);
        }
        self.instances.get_mut(&parent).expect("actor").join = None;
        let next = self.current_next(parent);
        self.advance(parent, payload, next.as_ref())
    }

    fn select_children(&self, id: ActorId, kind: Option<&str>) -> Vec<ActorId> {
        self.instances
            .get(&id)
            .map(|instance| {
                instance
                    .children
                    .iter()
                    .filter(|child| kind.is_none_or(|kind| child.component == kind))
                    .map(|child| child.id)
                    .collect()
            })
            .unwrap_or_default()
    }

    fn enqueue_send(
        &mut self,
        to: ActorId,
        message: &Json,
        after: Option<&Json>,
        route: Route,
        bindings: &HashMap<String, Json>,
        sends: &mut Vec<(ActorId, Json, Route)>,
    ) -> Result<(), Error> {
        let items = send_items(message);
        let mut slots = Vec::with_capacity(items.len());
        let mut any_async = false;
        for item in items {
            match self.eval_slot(item, bindings)? {
                Slot::Ready(value) => slots.push(Slot::Ready(value)),
                Slot::Async { delay_ms, value } => {
                    any_async = true;
                    slots.push(Slot::Async { delay_ms, value });
                }
            }
        }
        if any_async && after.is_some() {
            return Err(Error::new("send after cannot take an async argument"));
        }
        if !any_async {
            let value = assemble_slots(slots);
            if let Some(after) = after {
                let delay = self.delay_ms(after, bindings)?;
                self.timers.push(Timer {
                    due: self.now.saturating_add(delay),
                    to,
                    message: value,
                    route,
                    join: None,
                    slot: None,
                });
                return Ok(());
            }
            sends.push((to, value, route));
            return Ok(());
        }
        self.enqueue_join(to, route, slots)
    }

    fn enqueue_join(&mut self, to: ActorId, route: Route, slots: Vec<Slot>) -> Result<(), Error> {
        let join_id = self.next_join;
        self.next_join += 1;
        let mut filled = Vec::with_capacity(slots.len());
        let mut remaining = 0;
        let mut timers = Vec::new();
        for (index, slot) in slots.into_iter().enumerate() {
            match slot {
                Slot::Ready(value) => filled.push(Some(value)),
                Slot::Async { delay_ms, value } => {
                    remaining += 1;
                    filled.push(None);
                    timers.push(Timer {
                        due: self.now.saturating_add(delay_ms),
                        to,
                        message: value,
                        route,
                        join: Some(join_id),
                        slot: Some(index),
                    });
                }
            }
        }
        self.joins.insert(
            join_id,
            PendingJoin {
                remaining,
                slots: filled,
                to,
                route,
            },
        );
        self.timers.extend(timers);
        Ok(())
    }

    fn fill_join(&mut self, join_id: u64, slot: usize, value: Json) -> Result<Option<Json>, Error> {
        let Some(join) = self.joins.get_mut(&join_id) else {
            return Err(Error::new("unknown async join"));
        };
        if slot >= join.slots.len() {
            return Err(Error::new("async join slot"));
        }
        if join.slots[slot].is_some() {
            return Err(Error::new("async join slot already filled"));
        }
        join.slots[slot] = Some(value);
        join.remaining = join.remaining.saturating_sub(1);
        if join.remaining != 0 {
            return Ok(None);
        }
        let join = self.joins.remove(&join_id).expect("join");
        let values: Vec<Json> = join
            .slots
            .into_iter()
            .map(|slot| slot.expect("filled join"))
            .collect();
        Ok(Some(if values.len() == 1 {
            values.into_iter().next().expect("one")
        } else {
            Json::Array(values)
        }))
    }

    fn eval(&mut self, expression: &Json, bindings: &HashMap<String, Json>) -> Result<Json, Error> {
        match self.eval_slot(expression, bindings)? {
            Slot::Ready(value) => Ok(value),
            Slot::Async { .. } => Err(Error::new("async callee is only legal as a send payload")),
        }
    }

    fn eval_slot(
        &mut self,
        expression: &Json,
        bindings: &HashMap<String, Json>,
    ) -> Result<Slot, Error> {
        if expression.get("kind").and_then(Json::as_str) == Some("sequence") {
            let items = expression
                .get("items")
                .and_then(Json::as_array)
                .ok_or_else(|| Error::new("sequence items"))?;
            let mut values = Vec::with_capacity(items.len());
            for item in items {
                values.push(self.eval(item, bindings)?);
            }
            return Ok(Slot::Ready(Json::Array(values)));
        }
        if let Some(name) = invocation_name(expression) {
            let args = invocation_args(expression);
            match name {
                "rand" => {
                    let (lo, hi) = rand_args(expression)
                        .ok_or_else(|| Error::new("rand expected two arguments"))?;
                    let lo = self.eval(lo, bindings)?;
                    let hi = self.eval(hi, bindings)?;
                    return Ok(Slot::Ready(self.rand_between(&lo, &hi)?));
                }
                "write" | "writeLine" | "print" => {
                    let value = self.eval_write(name, args, bindings)?;
                    return Ok(Slot::Ready(value));
                }
                "http_get" => {
                    let url = args
                        .first()
                        .ok_or_else(|| Error::new("http_get expected a url"))?;
                    let url = self.eval(url, bindings)?;
                    return Ok(Slot::Async {
                        delay_ms: 1000,
                        value: json!({ "status": 200, "url": url }),
                    });
                }
                _ => {}
            }
        }
        Ok(Slot::Ready(evaluate(expression, bindings)?))
    }

    fn eval_write(
        &mut self,
        name: &str,
        args: &[Json],
        bindings: &HashMap<String, Json>,
    ) -> Result<Json, Error> {
        let payload = if name == "print" {
            self.eval_payload(args, bindings)?
        } else {
            let stream = args
                .first()
                .ok_or_else(|| Error::new(format!("{name} expected a stream")))?;
            let stream = self.eval(stream, bindings)?;
            check_stream(&stream, name)?;
            self.eval_payload(args.get(1..).unwrap_or(&[]), bindings)?
        };
        eprint!("{}", encode(&payload)?);
        self.prints.push(payload);
        Ok(Json::Null)
    }

    fn eval_payload(
        &mut self,
        args: &[Json],
        bindings: &HashMap<String, Json>,
    ) -> Result<Json, Error> {
        match args {
            [] => Ok(Json::Null),
            [one] => self.eval(one, bindings),
            many => {
                let mut values = Vec::with_capacity(many.len());
                for arg in many {
                    values.push(self.eval(arg, bindings)?);
                }
                Ok(Json::Array(values))
            }
        }
    }

    fn rand_between(&mut self, lo: &Json, hi: &Json) -> Result<Json, Error> {
        let lo = lo
            .as_f64()
            .ok_or_else(|| Error::new("rand expected numbers"))?;
        let hi = hi
            .as_f64()
            .ok_or_else(|| Error::new("rand expected numbers"))?;
        if lo > hi {
            return Err(Error::new("rand low bound is greater than high bound"));
        }
        self.rng = self.rng.wrapping_mul(1664525).wrapping_add(1013904223);
        if lo.fract() == 0.0 && hi.fract() == 0.0 {
            let width = (hi as i64 - lo as i64 + 1) as u64;
            let pick = lo as i64 + (self.rng % width) as i64;
            return Ok(json!(pick as f64));
        }
        let t = (self.rng as f64) / (u64::MAX as f64);
        Ok(json!(lo + (hi - lo) * t))
    }

    fn delay_ms(&mut self, after: &Json, bindings: &HashMap<String, Json>) -> Result<u64, Error> {
        let value = self.eval(after, bindings)?;
        let seconds = value
            .as_f64()
            .ok_or_else(|| Error::new("send after expected seconds"))?;
        if seconds < 0.0 {
            return Err(Error::new("send after delay must not be negative"));
        }
        Ok((seconds * 1000.0) as u64)
    }

    fn deliver(&mut self, to: ActorId, value: Json, route: Route) -> Result<(), Error> {
        self.route_seq += 1;
        self.routes.insert((to, self.route_seq), route);
        self.dispatcher.send(to, Message { value });
        Ok(())
    }

    fn take_route(&mut self, actor: ActorId) -> Route {
        let key = self
            .routes
            .keys()
            .filter(|(id, _)| *id == actor)
            .min_by_key(|(_, seq)| *seq)
            .copied();
        if let Some(key) = key {
            self.routes.remove(&key).unwrap_or(Route::Inject)
        } else {
            Route::Inject
        }
    }
}

fn route_matches(expected: &[String], route: Route) -> bool {
    if expected.is_empty() {
        return route != Route::Init;
    }
    expected.iter().any(|name| route_matches_one(name, route))
}

fn route_matches_one(name: &str, route: Route) -> bool {
    match name {
        "init" => route == Route::Init,
        "self" => route == Route::SelfSend,
        "up" => route == Route::Up,
        "down" => matches!(route, Route::Down | Route::Inject),
        _ => false,
    }
}

fn rand_args(expression: &Json) -> Option<(&Json, &Json)> {
    if invocation_name(expression) != Some("rand") {
        return None;
    }
    match invocation_args(expression) {
        [lo, hi] => Some((lo, hi)),
        _ => None,
    }
}

fn invocation_name(expression: &Json) -> Option<&str> {
    if expression.get("kind").and_then(Json::as_str) != Some("invocation") {
        return None;
    }
    expression
        .get("expression")
        .and_then(|value| value.get("name"))
        .and_then(Json::as_str)
}

fn invocation_args(expression: &Json) -> &[Json] {
    expression
        .get("args")
        .and_then(Json::as_array)
        .map(Vec::as_slice)
        .unwrap_or(&[])
}

fn send_items(message: &Json) -> Vec<&Json> {
    if message.get("kind").and_then(Json::as_str) == Some("sequence") {
        message
            .get("items")
            .and_then(Json::as_array)
            .map(|items| items.iter().collect())
            .unwrap_or_default()
    } else {
        vec![message]
    }
}

fn assemble_slots(slots: Vec<Slot>) -> Json {
    let mut values = Vec::with_capacity(slots.len());
    for slot in slots {
        match slot {
            Slot::Ready(value) => values.push(value),
            Slot::Async { .. } => unreachable!("assemble_slots of ready slots"),
        }
    }
    if values.len() == 1 {
        values.pop().expect("one")
    } else {
        Json::Array(values)
    }
}

fn stream(name: &str) -> Json {
    json!({ "kind": "stream", "name": name })
}

fn bind_world(bindings: &mut HashMap<String, Json>) {
    for name in ["stdin", "stdout", "stderr"] {
        bindings
            .entry(name.to_string())
            .or_insert_with(|| stream(name));
    }
}

fn check_stream(value: &Json, name: &str) -> Result<(), Error> {
    if value.get("kind").and_then(Json::as_str) != Some("stream") {
        return Err(Error::new(format!("{name} expected a stream")));
    }
    match value.get("name").and_then(Json::as_str) {
        Some("stdout") | Some("stderr") => Ok(()),
        Some("stdin") => Err(Error::new(format!("{name} cannot write stdin"))),
        _ => Err(Error::new(format!("{name} expected stdout or stderr"))),
    }
}

fn callee_of(name: &str) -> Callee {
    match name {
        "write" | "writeLine" | "print" => Callee::Io,
        "http_get" => Callee::Async,
        _ => Callee::Other,
    }
}

fn walk_callees(expression: &Json, visit: &mut impl FnMut(&str)) {
    if let Some(name) = invocation_name(expression) {
        visit(name);
    }
    match expression {
        Json::Object(map) => {
            for value in map.values() {
                walk_callees(value, visit);
            }
        }
        Json::Array(items) => {
            for value in items {
                walk_callees(value, visit);
            }
        }
        _ => {}
    }
}

fn check_sync(expression: &Json, component: &str) -> Result<(), Error> {
    let mut error = None;
    walk_callees(expression, &mut |name| {
        if error.is_some() {
            return;
        }
        match callee_of(name) {
            Callee::Io => {
                error = Some(format!(
                    "same-turn I/O cannot appear in a value in {component}"
                ));
            }
            Callee::Async => {
                error = Some(format!(
                    "async callee is only legal as a send payload in {component}"
                ));
            }
            Callee::Other => {}
        }
    });
    match error {
        Some(message) => Err(Error::new(message)),
        None => Ok(()),
    }
}

fn check_send_arg(expression: &Json, after: bool, component: &str) -> Result<(), Error> {
    if let Some(name) = invocation_name(expression) {
        match callee_of(name) {
            Callee::Async => {
                if after {
                    return Err(Error::new(format!(
                        "send after cannot take an async argument in {component}"
                    )));
                }
                for arg in invocation_args(expression) {
                    check_sync(arg, component)?;
                }
                return Ok(());
            }
            Callee::Io => {
                return Err(Error::new(format!(
                    "same-turn I/O is not a send payload in {component}"
                )));
            }
            Callee::Other => {}
        }
    }
    check_sync(expression, component)
}

fn check_expr_stmt(expression: &Json, component: &str) -> Result<(), Error> {
    if let Some(name) = invocation_name(expression) {
        match callee_of(name) {
            Callee::Io => {
                for arg in invocation_args(expression) {
                    check_sync(arg, component)?;
                }
                return Ok(());
            }
            Callee::Async => {
                return Err(Error::new(format!(
                    "async callee is only legal as a send payload in {component}"
                )));
            }
            Callee::Other => {}
        }
    }
    check_sync(expression, component)
}

fn check_effects(component: &crate::ast::Component) -> Result<(), Error> {
    for field in &component.fields {
        if let Some(init) = &field.init {
            check_sync(init, &component.name)?;
        }
    }
    for state in &component.states {
        for handler in &state.handlers {
            for statement in &handler.body {
                match statement {
                    Statement::Assign { expression, .. } => {
                        check_sync(expression, &component.name)?;
                    }
                    Statement::Expr { expression } => {
                        check_expr_stmt(expression, &component.name)?;
                    }
                    Statement::Complete { message } => {
                        check_sync(message, &component.name)?;
                    }
                    Statement::Send { message, after }
                    | Statement::SendUp { message, after }
                    | Statement::SendTo { message, after, .. }
                    | Statement::SendDown { message, after, .. } => {
                        let delayed = after.is_some();
                        for item in send_items(message) {
                            check_send_arg(item, delayed, &component.name)?;
                        }
                    }
                    Statement::Goto { .. } => {}
                }
            }
        }
    }
    Ok(())
}

fn bind_step(pattern: &Json, working: &Json) -> Result<HashMap<String, Json>, Error> {
    match_message(pattern, working)?.ok_or_else(|| Error::new("step pattern missed"))
}

fn match_handler(
    handler: &crate::ast::Handler,
    message: &Json,
) -> Result<Option<HashMap<String, Json>>, Error> {
    let Some(item_pattern) = &handler.item else {
        return match_message(&handler.pattern, message);
    };
    let Some([acc, item]) = message.as_array().map(Vec::as_slice) else {
        return Ok(None);
    };
    let Some(mut bindings) = match_message(&handler.pattern, acc)? else {
        return Ok(None);
    };
    let Some(item_bindings) = match_message(item_pattern, item)? else {
        return Ok(None);
    };
    bindings.extend(item_bindings);
    Ok(Some(bindings))
}

fn bindings_from(value: &Json) -> HashMap<String, Json> {
    match value {
        Json::Object(map) => map.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
        _ => HashMap::new(),
    }
}

fn workflow_from_value(value: Json) -> Result<Workflow, Error> {
    if value.get("kind").and_then(Json::as_str) == Some("task") {
        let component = value
            .get("component")
            .and_then(Json::as_str)
            .ok_or_else(|| Error::new("dynamic task missing component"))?
            .to_string();
        return Ok(Workflow {
            kind: "workflow".into(),
            name: "Dynamic".into(),
            start: "Start".into(),
            steps: vec![FlowStep {
                name: "Start".into(),
                start: true,
                pattern: None,
                body: StepBody::Task { component },
                next: Some(Next::Succeed {}),
            }],
        });
    }
    serde_json::from_value(value).map_err(|error| Error::new(error.to_string()))
}

fn unmatched(
    route: Route,
    kind: &str,
    parent: Option<ActorId>,
    message: &Json,
) -> Result<Vec<(ActorId, Json, Route)>, Error> {
    match route {
        Route::Up => {
            if let Some(parent) = parent {
                Ok(vec![(parent, message.clone(), Route::Up)])
            } else {
                Ok(vec![])
            }
        }
        Route::Down => Err(Error::new(format!("unmatched down to {kind}"))),
        Route::Inject => Err(Error::new("unmatched injected message")),
        Route::Join => Err(Error::new(format!("no join handler on {kind}"))),
        Route::Init => Ok(vec![]),
        Route::SelfSend => Err(Error::new(format!("unmatched self on {kind}"))),
    }
}

fn check_gotos(component: &crate::ast::Component) -> Result<(), Error> {
    let names: HashSet<_> = component
        .states
        .iter()
        .filter(|state| state.named)
        .map(|state| state.name.as_str())
        .collect();
    for state in &component.states {
        for handler in &state.handlers {
            for statement in &handler.body {
                if let Statement::Goto { state: target } = statement {
                    if !names.contains(target.as_str()) {
                        return Err(Error::new(format!(
                            "unknown state {target} in {}",
                            component.name
                        )));
                    }
                }
            }
        }
    }
    Ok(())
}

fn check_terminals(component: &crate::ast::Component) -> Result<(), Error> {
    for state in &component.states {
        for handler in &state.handlers {
            let last = handler.body.len().saturating_sub(1);
            for (index, statement) in handler.body.iter().enumerate() {
                let terminal = matches!(
                    statement,
                    Statement::Goto { .. } | Statement::Complete { .. }
                );
                if terminal && index != last {
                    return Err(Error::new(format!(
                        "goto and complete must be last in {}",
                        component.name
                    )));
                }
            }
        }
    }
    Ok(())
}

fn check_routes(component: &crate::ast::Component) -> Result<(), Error> {
    for state in &component.states {
        for handler in &state.handlers {
            let mut seen = HashSet::new();
            for name in &handler.route {
                match name.as_str() {
                    "init" => {
                        if handler.route.len() != 1 {
                            return Err(Error::new(format!(
                                "init cannot be combined with other routes in {}",
                                component.name
                            )));
                        }
                    }
                    "up" | "down" | "self" => {}
                    other => {
                        return Err(Error::new(format!(
                            "unknown route {other} in {}",
                            component.name
                        )));
                    }
                }
                if !seen.insert(name.as_str()) {
                    return Err(Error::new(format!(
                        "duplicate route {name} in {}",
                        component.name
                    )));
                }
            }
        }
    }
    Ok(())
}

fn check_ports(program: &Program, ports: &[crate::ast::Port]) -> Result<(), Error> {
    let mut port_names = HashSet::new();
    for port in ports {
        if !port_names.insert(port.name.as_str()) {
            return Err(Error::new(format!("duplicate port {}", port.name)));
        }
        if program.kind(&port.component).is_none() {
            return Err(Error::new(format!("unknown port kind {}", port.component)));
        }
    }
    Ok(())
}

fn check_workflow(program: &Program, workflow: &Workflow) -> Result<(), Error> {
    if workflow.start_step().is_none() {
        return Err(Error::new(format!(
            "workflow {} must declare exactly one start step",
            workflow.name
        )));
    }
    let names: HashSet<_> = workflow
        .steps
        .iter()
        .map(|step| step.name.as_str())
        .collect();
    for step in &workflow.steps {
        match &step.body {
            StepBody::Task { component } => {
                if program.kind(component).is_none() {
                    return Err(Error::new(format!("unknown step kind {component}")));
                }
            }
            StepBody::Map {
                component, reduce, ..
            } => {
                for kind in [component, reduce] {
                    if program.kind(kind).is_none() {
                        return Err(Error::new(format!("unknown step kind {kind}")));
                    }
                }
            }
            StepBody::Choice { clauses } => {
                for clause in clauses {
                    if !names.contains(clause.step.as_str()) {
                        return Err(Error::new(format!("unknown step {}", clause.step)));
                    }
                }
            }
            _ => {}
        }
        if let Some(Next::Goto { step }) = &step.next {
            if !names.contains(step.as_str()) {
                return Err(Error::new(format!("unknown step {step}")));
            }
        }
    }
    Ok(())
}

fn check_instance(program: &Program, node: &Instance) -> Result<(), Error> {
    if let Some(workflow) = program.workflow(&node.component) {
        let prototypes: HashSet<&str> = node
            .children
            .iter()
            .map(|child| child.component.as_str())
            .collect();
        for step in &workflow.steps {
            let kinds = match &step.body {
                StepBody::Task { component } => vec![component.as_str()],
                StepBody::Map {
                    component, reduce, ..
                } => vec![component.as_str(), reduce.as_str()],
                _ => vec![],
            };
            for component in kinds {
                if let Some(kind) = program.kind(component) {
                    if !kind.ports.is_empty() && !prototypes.contains(component) {
                        return Err(Error::new(format!(
                            "fanout kind {component} must not declare ports"
                        )));
                    }
                }
            }
        }
        for child in &node.children {
            check_instance(program, child)?;
        }
        return Ok(());
    }
    let ports: Vec<(String, String)> = {
        let kind = program
            .kind(&node.component)
            .ok_or_else(|| Error::new(format!("unknown kind {}", node.component)))?;
        if !kind.ports.is_empty() && node.children.is_empty() {
            // spawned bare by a map/task without prototype — checked at spawn/check_workflow
        }
        kind.ports
            .iter()
            .map(|port| (port.name.clone(), port.component.clone()))
            .collect()
    };
    if !ports.is_empty() {
        let mut filled = HashSet::new();
        for child in &node.children {
            if let Some(key) = child.key.as_deref() {
                if !filled.insert(key.to_string()) {
                    return Err(Error::new(format!("duplicate port fill {key}")));
                }
                match ports.iter().find(|(name, _)| name == key) {
                    None => return Err(Error::new(format!("unknown port {key}"))),
                    Some((_, expected)) if expected != &child.component => {
                        return Err(Error::new(format!(
                            "port {key} expects {expected} but scene filled {}",
                            child.component
                        )));
                    }
                    Some(_) => {}
                }
            }
            check_instance(program, child)?;
        }
        for (name, _) in &ports {
            if !filled.contains(name) {
                return Err(Error::new(format!("missing port fill {name}")));
            }
        }
    } else {
        for child in &node.children {
            check_instance(program, child)?;
        }
    }
    Ok(())
}

fn snapshot_route(route: Route) -> &'static str {
    match route {
        Route::Inject => "inject",
        Route::Down => "down",
        Route::Up => "up",
        Route::Join => "join",
        Route::Init => "init",
        Route::SelfSend => "self",
    }
}

fn restore_route(value: Option<&str>) -> Route {
    match value {
        Some("down") => Route::Down,
        Some("up") => Route::Up,
        Some("join") => Route::Join,
        Some("init") => Route::Init,
        Some("self") => Route::SelfSend,
        _ => Route::Inject,
    }
}

fn restore_timer(entry: &Json) -> Result<Timer, Error> {
    Ok(Timer {
        due: entry
            .get("due")
            .and_then(json_u64)
            .ok_or_else(|| Error::new("timer missing due"))?,
        to: ActorId(
            entry
                .get("to")
                .and_then(json_u64)
                .ok_or_else(|| Error::new("timer missing to"))?,
        ),
        message: entry
            .get("message")
            .cloned()
            .ok_or_else(|| Error::new("timer missing message"))?,
        route: restore_route(entry.get("route").and_then(Json::as_str)),
        join: entry.get("join").and_then(json_u64),
        slot: entry
            .get("slot")
            .and_then(json_u64)
            .map(|slot| slot as usize),
    })
}

fn restore_join(entry: &Json) -> Result<PendingJoin, Error> {
    let remaining = entry
        .get("remaining")
        .and_then(json_u64)
        .ok_or_else(|| Error::new("join missing remaining"))? as usize;
    let slots = entry
        .get("slots")
        .and_then(Json::as_array)
        .ok_or_else(|| Error::new("join missing slots"))?
        .iter()
        .map(|slot| {
            if slot.is_null() {
                None
            } else {
                Some(slot.clone())
            }
        })
        .collect();
    Ok(PendingJoin {
        remaining,
        slots,
        to: ActorId(
            entry
                .get("to")
                .and_then(json_u64)
                .ok_or_else(|| Error::new("join missing to"))?,
        ),
        route: restore_route(entry.get("route").and_then(Json::as_str)),
    })
}

fn restore_instance(entry: &Json) -> Result<InstanceState, Error> {
    let kind = entry
        .get("kind")
        .and_then(Json::as_str)
        .ok_or_else(|| Error::new("instance missing kind"))?
        .to_string();
    let parent = entry.get("parent").and_then(json_u64).map(ActorId);
    let mut children = Vec::new();
    if let Some(list) = entry.get("children").and_then(Json::as_array) {
        for child in list {
            children.push(ChildRef {
                id: ActorId(
                    child
                        .get("id")
                        .and_then(json_u64)
                        .ok_or_else(|| Error::new("child missing id"))?,
                ),
                key: child.get("key").and_then(Json::as_str).map(str::to_string),
                component: child
                    .get("component")
                    .and_then(Json::as_str)
                    .unwrap_or("")
                    .to_string(),
            });
        }
    }
    let fields = entry
        .get("fields")
        .and_then(Json::as_object)
        .map(|map| map.iter().map(|(k, v)| (k.clone(), v.clone())).collect())
        .unwrap_or_default();
    let join = entry
        .get("join")
        .filter(|value| !value.is_null())
        .map(|value| {
            let remaining = value
                .get("remaining")
                .and_then(Json::as_array)
                .map(|ids| ids.iter().filter_map(json_u64).map(ActorId).collect())
                .unwrap_or_default();
            JoinSet {
                remaining,
                total: value.get("total").and_then(json_u64).unwrap_or(0),
                completed: value.get("completed").and_then(json_u64).unwrap_or(0),
                reducer: value.get("reducer").and_then(json_u64).map(ActorId),
            }
        });
    let run = match entry.get("run") {
        Some(value) if !value.is_null() => Some(restore_run(value)?),
        _ => None,
    };
    Ok(InstanceState {
        kind,
        parent,
        children,
        fields,
        state: entry
            .get("state")
            .and_then(Json::as_str)
            .unwrap_or("")
            .to_string(),
        completed: entry
            .get("completed")
            .and_then(Json::as_bool)
            .unwrap_or(false),
        completion: entry
            .get("completion")
            .cloned()
            .filter(|value| !value.is_null()),
        join,
        run,
    })
}

fn restore_run(value: &Json) -> Result<WorkflowRun, Error> {
    let definition: Workflow = serde_json::from_value(
        value
            .get("definition")
            .cloned()
            .ok_or_else(|| Error::new("run missing definition"))?,
    )
    .map_err(|error| Error::new(error.to_string()))?;
    let prototypes = serde_json::from_value(
        value
            .get("prototypes")
            .cloned()
            .unwrap_or_else(|| json!([])),
    )
    .map_err(|error| Error::new(error.to_string()))?;
    Ok(WorkflowRun {
        status: match value.get("status").and_then(Json::as_str) {
            Some("succeeded") => RunStatus::Succeeded,
            Some("failed") => RunStatus::Failed,
            _ => RunStatus::Running,
        },
        step: value
            .get("step")
            .and_then(Json::as_str)
            .unwrap_or("")
            .to_string(),
        input: value.get("input").cloned().unwrap_or(Json::Null),
        output: value.get("output").cloned().unwrap_or(Json::Null),
        definition,
        prototypes,
    })
}
