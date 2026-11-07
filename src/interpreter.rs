use crate::ast::{FlowStep, Instance, Next, Program, Statement, StepBody, Workflow};
use crate::codec::{decode, encode, json_u64};
use crate::dispatch::{ActorId, Dispatcher, Message};
use crate::error::Error;
use crate::host::{evaluate, match_message};
use serde_json::{json, Value as Json};
use std::collections::{HashMap, HashSet, VecDeque};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    Inject,
    Down,
    Up,
    Join,
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
    acc: Json,
    reducer: Option<String>,
    pending: VecDeque<Json>,
    folding: Option<ActorId>,
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
        };
        world.spawn_instance(&scene.root, None)?;
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
                        "acc": join.acc,
                        "reducer": join.reducer,
                        "pending": join.pending.iter().cloned().collect::<Vec<_>>(),
                        "folding": join.folding.map(|ActorId(id)| id),
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
                    "route": match route {
                        Route::Inject => "inject",
                        Route::Down => "down",
                        Route::Up => "up",
                        Route::Join => "join",
                    }
                })
            })
            .collect();
        json!({
            "nextId": self.next_id,
            "routeSeq": self.route_seq,
            "instances": instances,
            "dispatcher": self.dispatcher.snapshot(),
            "routes": routes,
            "prints": self.prints,
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
                let route = match entry.get("route").and_then(Json::as_str) {
                    Some("down") => Route::Down,
                    Some("up") => Route::Up,
                    Some("join") => Route::Join,
                    _ => Route::Inject,
                };
                routes.insert((id, seq), route);
            }
        }
        let prints = snapshot
            .get("prints")
            .and_then(Json::as_array)
            .cloned()
            .unwrap_or_default();
        Ok(Self {
            program,
            instances,
            next_id,
            dispatcher,
            routes,
            route_seq,
            prints,
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
                evaluate(init, &HashMap::new())?
            } else {
                Json::Null
            };
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
        match &step.body {
            StepBody::Succeed {} => self.succeed(id, working),
            StepBody::Fail { error } => {
                let value = evaluate(error, &bindings_from(&working))?;
                self.fail(id, value)
            }
            StepBody::Pass { value } => {
                let output = evaluate(value, &bindings_from(&working))?;
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
            } => self.spawn_map(id, items, component, reduce, init, working),
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
                self.arm_join(id, [child].into_iter().collect(), 1, None, Json::Null);
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
        self.arm_join(id, [child].into_iter().collect(), 1, None, Json::Null);
        self.start_work(child, working)
    }

    fn spawn_map(
        &mut self,
        id: ActorId,
        items_path: &str,
        component: &str,
        reduce: &str,
        init: &Json,
        working: Json,
    ) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        let acc = evaluate(init, &bindings_from(&working))?;
        let items = working
            .get(items_path)
            .and_then(Json::as_array)
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
        if children.is_empty() {
            let next = self.current_next(id);
            return self.advance(id, acc, next.as_ref());
        }
        let remaining: HashSet<_> = children.iter().map(|(child, _)| *child).collect();
        let total = remaining.len() as u64;
        self.arm_join(id, remaining, total, Some(reduce.to_string()), acc);
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

    fn arm_join(
        &mut self,
        id: ActorId,
        remaining: HashSet<ActorId>,
        total: u64,
        reducer: Option<String>,
        acc: Json,
    ) {
        self.instances.get_mut(&id).expect("actor").join = Some(JoinSet {
            remaining,
            total,
            completed: 0,
            acc,
            reducer,
            pending: VecDeque::new(),
            folding: None,
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
                let value = evaluate(error, &bindings_from(&output))?;
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
                Route::Up => Ok(vec![]),
                Route::Down => Err(Error::new("workflow does not take down messages")),
            };
        }
        let (kind_name, state_name, fields) = {
            let instance = self
                .instances
                .get(&id)
                .ok_or_else(|| Error::new("unknown actor"))?;
            (
                instance.kind.clone(),
                instance.state.clone(),
                instance.fields.clone(),
            )
        };
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
        for handler in &state.handlers {
            if let Some(mut bindings) = match_message(&handler.pattern, message)? {
                for (name, value) in &fields {
                    bindings.entry(name.clone()).or_insert(value.clone());
                }
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
                    let value = evaluate(expression, &bindings)?;
                    bindings.insert(field.clone(), value.clone());
                    self.instances
                        .get_mut(&id)
                        .expect("actor")
                        .fields
                        .insert(field.clone(), value);
                }
                Statement::SendUp { message } => {
                    let value = evaluate(message, &bindings)?;
                    let parent = self.instances[&id]
                        .parent
                        .ok_or_else(|| Error::new("send up from root"))?;
                    sends.push((parent, value, Route::Up));
                }
                Statement::SendTo { port, message } => {
                    let value = evaluate(message, &bindings)?;
                    let child = self.instances[&id]
                        .children
                        .iter()
                        .find(|child| child.key.as_deref() == Some(port.as_str()))
                        .ok_or_else(|| Error::new(format!("unknown port {port}")))?;
                    sends.push((child.id, value, Route::Down));
                }
                Statement::SendDown { component, message } => {
                    let value = evaluate(message, &bindings)?;
                    let selected = self.select_children(id, component.as_deref());
                    if selected.is_empty() {
                        return Err(Error::new("no child matched send down"));
                    }
                    for child_id in selected {
                        sends.push((child_id, value.clone(), Route::Down));
                    }
                }
                Statement::Print { expression } => {
                    let value = evaluate(expression, &bindings)?;
                    eprint!("{}", encode(&value)?);
                    self.prints.push(value);
                }
                Statement::Complete { message } => {
                    let value = evaluate(message, &bindings)?;
                    {
                        let instance = self.instances.get_mut(&id).expect("actor");
                        instance.completed = true;
                        instance.completion = Some(value.clone());
                    }
                    sends.extend(self.finish_to_parent(id, value)?);
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
        let Some(join) = self.instances.get(&parent).and_then(|instance| instance.join.as_ref())
        else {
            return Ok(vec![]);
        };
        if join.folding == Some(child) {
            if let Some(join) = self
                .instances
                .get_mut(&parent)
                .and_then(|instance| instance.join.as_mut())
            {
                join.acc = payload;
                join.folding = None;
            }
            return self.pump_reduce(parent);
        }
        let reduce = join.reducer.is_some();
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
            if reduce {
                join.pending.push_back(payload.clone());
            }
        }
        if reduce {
            return self.pump_reduce(parent);
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

    fn pump_reduce(
        &mut self,
        parent: ActorId,
    ) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        let (reducer, item, acc, finished) = {
            let join = match self
                .instances
                .get_mut(&parent)
                .and_then(|instance| instance.join.as_mut())
            {
                Some(join) => join,
                None => return Ok(vec![]),
            };
            if join.folding.is_some() {
                return Ok(vec![]);
            }
            if let Some(item) = join.pending.pop_front() {
                let reducer = join
                    .reducer
                    .clone()
                    .ok_or_else(|| Error::new("map join missing reducer"))?;
                (reducer, Some(item), join.acc.clone(), None)
            } else if join.remaining.is_empty() {
                (String::new(), None, Json::Null, Some(join.acc.clone()))
            } else {
                return Ok(vec![]);
            }
        };
        if let Some(acc) = finished {
            self.instances.get_mut(&parent).expect("actor").join = None;
            let next = self.current_next(parent);
            return self.advance(parent, acc, next.as_ref());
        }
        let child = self.spawn_work(parent, &reducer)?;
        if let Some(join) = self
            .instances
            .get_mut(&parent)
            .and_then(|instance| instance.join.as_mut())
        {
            join.folding = Some(child);
        }
        self.start_work(
            child,
            json!({
                "kind": "Reduce",
                "acc": acc,
                "item": item.expect("pending item"),
            }),
        )
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
    }
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
                acc: value.get("acc").cloned().unwrap_or(Json::Null),
                reducer: value
                    .get("reducer")
                    .and_then(Json::as_str)
                    .map(str::to_string),
                pending: value
                    .get("pending")
                    .and_then(Json::as_array)
                    .cloned()
                    .unwrap_or_default()
                    .into(),
                folding: value.get("folding").and_then(json_u64).map(ActorId),
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
