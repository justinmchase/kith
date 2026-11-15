use crate::ast::{Field, FlowStep, Instance, Next, Program, Statement, StepBody, Workflow};
use crate::builtin::{self, Driver};
use crate::dispatch::{ActorId, Dispatcher, Message};
use crate::doc::{self, Doc};
use crate::error::Error;
use crate::host::{evaluate_document, match_document, match_field_document};
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
    message: Doc,
    route: Route,
    join: Option<u64>,
    slot: Option<usize>,
}

struct PendingJoin {
    remaining: usize,
    slots: Vec<Option<Doc>>,
    to: ActorId,
    route: Route,
}

enum Slot {
    Ready(Doc),
    Async { delay_ms: u64, value: Doc },
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
    input: Doc,
    output: Doc,
    output_json: Json,
    definition: Workflow,
    prototypes: Vec<Instance>,
}

struct InstanceState {
    kind: String,
    parent: Option<ActorId>,
    children: Vec<ChildRef>,
    fields: HashMap<String, Doc>,
    field_json: HashMap<String, Json>,
    state: String,
    completed: bool,
    completion: Option<Doc>,
    completion_json: Option<Json>,
    join: Option<JoinSet>,
    run: Option<WorkflowRun>,
    driver: Option<Driver>,
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
    clock_zone: Doc,
    clock_script: Option<Vec<Doc>>,
    clock_replies: Vec<Doc>,
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
            clock_zone: builtin::process_zone(),
            clock_script: None,
            clock_replies: Vec::new(),
        };
        world.spawn_instance(&scene.root, None)?;
        world.enqueue_inits()?;
        Ok(world)
    }

    pub fn root_id(&self) -> ActorId {
        ActorId(1)
    }

    pub fn field(&self, id: ActorId, name: &str) -> Option<&Json> {
        self.instances.get(&id)?.field_json.get(name)
    }

    pub fn output(&self, id: ActorId) -> Option<&Json> {
        let instance = self.instances.get(&id)?;
        if let Some(run) = &instance.run {
            return Some(&run.output_json);
        }
        instance.completion_json.as_ref()
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
        self.instances.get(&id)?.completion_json.as_ref()
    }

    pub fn progress(&self, id: ActorId) -> Option<Json> {
        let join = self.instances.get(&id)?.join.as_ref()?;
        Some(json!({
            "completed": join.completed,
            "total": join.total,
        }))
    }

    pub fn result_document(&self) -> Doc {
        if let Some(output) = self.output_document(self.root_id()) {
            return output;
        }
        let fields = self
            .instances
            .get(&self.root_id())
            .map(|instance| instance.fields.clone())
            .unwrap_or_default();
        doc::object(fields)
    }

    fn output_document(&self, id: ActorId) -> Option<Doc> {
        let instance = self.instances.get(&id)?;
        if let Some(run) = &instance.run {
            return Some(run.output.clone());
        }
        instance.completion.clone()
    }

    pub fn prints(&self) -> &[Json] {
        &self.prints
    }

    pub fn virtual_now(&self) -> u64 {
        self.now
    }

    /// Replace the wall-clock script. `zone` is frozen for later `"Zone"`
    /// replies. Each script entry is one `"Now"` instant, in order. An empty
    /// script makes the next `"Now"` fail. This does not move the dispatcher
    /// clock that `send after` uses.
    pub fn replace_clock(&mut self, zone: &str, script: &[&str]) -> Result<(), Error> {
        let zone = builtin::zone_value(zone)?;
        let mut instants = Vec::with_capacity(script.len());
        for text in script {
            let millis = builtin::parse_instant(text)?;
            instants.push(builtin::instant_doc(millis));
        }
        self.clock_zone = zone;
        self.clock_script = Some(instants);
        self.clock_replies.clear();
        Ok(())
    }

    /// External edge: `kith run --message` and existing tests pass one JSON value.
    pub fn send(&mut self, id: ActorId, message: Json) -> Result<(), Error> {
        self.send_document(id, doc::from_json(&message)?)
    }

    pub fn send_document(&mut self, id: ActorId, message: Doc) -> Result<(), Error> {
        self.deliver(id, message, Route::Down)
    }

    /// External edge: `kith run --message` passes one JSON value.
    pub fn inject(&mut self, message: Json) -> Result<(), Error> {
        let message = doc::from_json(&message)?;
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
        doc::encode(&self.snapshot_document()?)
    }

    fn snapshot_document(&self) -> Result<Doc, Error> {
        let instances = doc::array(
            self.instances
                .iter()
                .map(|(ActorId(id), instance)| instance_document(*id, instance))
                .collect::<Result<Vec<_>, _>>()?,
        );
        let routes = doc::array(self.routes.iter().map(|((ActorId(id), seq), route)| {
            doc::object([
                ("id", doc::number(*id as f64)),
                ("seq", doc::number(*seq as f64)),
                ("route", doc::string(snapshot_route(*route))),
            ])
        }));
        let joins = doc::array(self.joins.iter().map(|(id, join)| {
            doc::object([
                ("id", doc::number(*id as f64)),
                ("remaining", doc::number(join.remaining as f64)),
                (
                    "slots",
                    doc::array(
                        join.slots
                            .iter()
                            .map(|slot| slot.clone().unwrap_or_else(doc::null)),
                    ),
                ),
                ("to", doc::number(join.to.0 as f64)),
                ("route", doc::string(snapshot_route(join.route))),
            ])
        }));
        let timers = doc::array(self.timers.iter().map(|timer| {
            doc::object([
                ("due", doc::number(timer.due as f64)),
                ("to", doc::number(timer.to.0 as f64)),
                ("message", timer.message.clone()),
                ("route", doc::string(snapshot_route(timer.route))),
                (
                    "join",
                    timer
                        .join
                        .map(|id| doc::number(id as f64))
                        .unwrap_or_else(doc::null),
                ),
                (
                    "slot",
                    timer
                        .slot
                        .map(|slot| doc::number(slot as f64))
                        .unwrap_or_else(doc::null),
                ),
            ])
        }));
        // The in-memory print log is the JSON observation tests already read.
        // It is embedded in the snapshot document at this persistence edge.
        let prints = doc::array(
            self.prints
                .iter()
                .map(doc::from_json)
                .collect::<Result<Vec<_>, _>>()?,
        );
        Ok(doc::object([
            ("nextId", doc::number(self.next_id as f64)),
            ("routeSeq", doc::number(self.route_seq as f64)),
            ("now", doc::number(self.now as f64)),
            ("rng", doc::number(self.rng as f64)),
            ("instances", instances),
            ("dispatcher", self.dispatcher.snapshot()),
            ("routes", routes),
            ("prints", prints),
            ("nextJoin", doc::number(self.next_join as f64)),
            ("joins", joins),
            ("timers", timers),
            (
                "clock",
                doc::object([
                    ("zone", self.clock_zone.clone()),
                    (
                        "script",
                        self.clock_script
                            .clone()
                            .map(doc::array)
                            .unwrap_or_else(doc::null),
                    ),
                    ("replies", doc::array(self.clock_replies.clone())),
                ]),
            ),
        ]))
    }

    pub fn restore(program: Program, snapshot: &str) -> Result<Self, Error> {
        Self::check(&program)?;
        let snapshot = doc::decode(snapshot)?;
        let next_id = doc::as_u64(&doc::get(&snapshot, "nextId").unwrap_or_else(doc::null))
            .ok_or_else(|| Error::new("snapshot missing nextId"))?;
        let route_seq =
            doc::as_u64(&doc::get(&snapshot, "routeSeq").unwrap_or_else(doc::null)).unwrap_or(0);
        let dispatcher = Dispatcher::restore(
            &doc::get(&snapshot, "dispatcher")
                .ok_or_else(|| Error::new("snapshot missing dispatcher"))?,
        )
        .map_err(Error::new)?;
        let mut instances = HashMap::new();
        for entry in doc::items(&doc::get(&snapshot, "instances").unwrap_or_else(doc::null))
            .ok_or_else(|| Error::new("snapshot missing instances"))?
        {
            let id = ActorId(
                doc::as_u64(&doc::get(&entry, "id").unwrap_or_else(doc::null))
                    .ok_or_else(|| Error::new("instance missing id"))?,
            );
            instances.insert(id, restore_instance(&entry)?);
        }
        let mut routes = HashMap::new();
        if let Some(list) = doc::items(&doc::get(&snapshot, "routes").unwrap_or_else(doc::null)) {
            for entry in list {
                let id = ActorId(
                    doc::as_u64(&doc::get(&entry, "id").unwrap_or_else(doc::null))
                        .ok_or_else(|| Error::new("route missing id"))?,
                );
                let seq =
                    doc::as_u64(&doc::get(&entry, "seq").unwrap_or_else(doc::null)).unwrap_or(0);
                let route = restore_route(
                    doc::as_str(&doc::get(&entry, "route").unwrap_or_else(doc::null)).as_deref(),
                );
                routes.insert((id, seq), route);
            }
        }
        let prints = doc::items(&doc::get(&snapshot, "prints").unwrap_or_else(doc::null))
            .unwrap_or_default()
            .iter()
            .map(doc::to_json)
            .collect::<Result<Vec<_>, _>>()?;
        let now = doc::as_u64(&doc::get(&snapshot, "now").unwrap_or_else(doc::null)).unwrap_or(0);
        let rng = doc::as_u64(&doc::get(&snapshot, "rng").unwrap_or_else(doc::null)).unwrap_or(1);
        let next_join =
            doc::as_u64(&doc::get(&snapshot, "nextJoin").unwrap_or_else(doc::null)).unwrap_or(1);
        let mut timers = Vec::new();
        if let Some(list) = doc::items(&doc::get(&snapshot, "timers").unwrap_or_else(doc::null)) {
            for entry in list {
                timers.push(restore_timer(&entry)?);
            }
        }
        let mut joins = HashMap::new();
        if let Some(list) = doc::items(&doc::get(&snapshot, "joins").unwrap_or_else(doc::null)) {
            for entry in list {
                let id = doc::as_u64(&doc::get(&entry, "id").unwrap_or_else(doc::null))
                    .ok_or_else(|| Error::new("join missing id"))?;
                joins.insert(id, restore_join(&entry)?);
            }
        }
        let clock = doc::get(&snapshot, "clock");
        let clock_zone = clock
            .as_ref()
            .and_then(|clock| doc::get(clock, "zone"))
            .unwrap_or_else(|| doc::object([("kind", doc::string("utc"))]));
        let clock_script = clock.as_ref().and_then(|clock| {
            let script = doc::get(clock, "script")?;
            if doc::is_null(&script) {
                None
            } else {
                doc::items(&script)
            }
        });
        let clock_replies = clock
            .as_ref()
            .and_then(|clock| doc::get(clock, "replies"))
            .and_then(|replies| doc::items(&replies))
            .unwrap_or_default();
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
            clock_zone,
            clock_script,
            clock_replies,
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
            self.deliver(id, doc::null(), Route::Init)?;
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
                doc::null()
            };
            self.match_declared_field(&component.fields, &field.name, &value)?;
            fields.insert(field.name.clone(), value);
        }
        let field_json = project_map(&fields)?;
        self.instances.insert(
            id,
            InstanceState {
                kind: kind.to_string(),
                parent,
                children: Vec::new(),
                fields,
                field_json,
                state: start,
                completed: false,
                completion: None,
                completion_json: None,
                join: None,
                run: None,
                driver: self.driver_for(kind),
            },
        );
        Ok(id)
    }

    fn driver_for(&self, kind: &str) -> Option<Driver> {
        self.program
            .host_kinds
            .iter()
            .any(|name| name == kind)
            .then(|| builtin::initial_driver(kind))
            .flatten()
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
                field_json: HashMap::new(),
                state: start.clone(),
                completed: false,
                completion: None,
                completion_json: None,
                join: None,
                run: Some(WorkflowRun {
                    status: RunStatus::Running,
                    step: start,
                    input: doc::null(),
                    output: doc::null(),
                    output_json: Json::Null,
                    definition: workflow,
                    prototypes: Vec::new(),
                }),
                driver: None,
            },
        );
        Ok(id)
    }

    fn start_run(&mut self, id: ActorId, input: Doc) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
        if let Some(run) = self
            .instances
            .get_mut(&id)
            .and_then(|instance| instance.run.as_mut())
        {
            run.input = input.clone();
            run.output = input.clone();
            run.output_json = project(&input)?;
            run.status = RunStatus::Running;
        }
        self.enter_current(id)
    }

    fn enter_current(&mut self, id: ActorId) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
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
        working: Doc,
    ) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
        if let Some(run) = self
            .instances
            .get_mut(&id)
            .and_then(|instance| instance.run.as_mut())
        {
            run.step = step.name.clone();
            run.output = working.clone();
        }
        let bindings = match &step.pattern {
            Some(pattern) => bind_step(pattern, &working, &self.program.rules)?,
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
                    if match_program(&clause.pattern, &working, &self.program.rules)?.is_some() {
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
                let plan = doc::get(&working, path)
                    .ok_or_else(|| Error::new(format!("run path {path} missing")))?;
                // A dynamic workflow plan is a Kith syntax tree.
                let definition = workflow_from_value(doc::to_json(&plan)?)?;
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
        working: Doc,
    ) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
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
        bindings: &HashMap<String, Doc>,
    ) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
        let reducer = self.spawn_work(id, reduce)?;
        if let Some(init) = init {
            let value = self.eval(init, bindings)?;
            self.apply_fields(reducer, value)?;
        }
        let items = self.eval(items_expr, bindings)?;
        let items = doc::items(&items).ok_or_else(|| Error::new("fanout expects a list"))?;
        let mut children = Vec::new();
        for (index, item) in items.into_iter().enumerate() {
            let child = self.spawn_work(id, component)?;
            children.push((
                child,
                doc::object([
                    ("kind", doc::string("Item")),
                    ("item", item),
                    ("index", doc::number(index as f64)),
                ]),
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
        message: Doc,
    ) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
        if self.instances[&child].run.is_some() {
            self.start_run(child, message)
        } else {
            Ok(vec![(child, message, Route::Down)])
        }
    }

    fn end_accumulator(&mut self, child: ActorId) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
        Ok(vec![(child, doc::null(), Route::Join)])
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

    fn match_declared_field(&self, fields: &[Field], name: &str, value: &Doc) -> Result<(), Error> {
        let field = fields
            .iter()
            .find(|field| field.name == name)
            .ok_or_else(|| Error::new(format!("unknown field {name}")))?;
        let Some(pattern) = &field.pattern else {
            return Ok(());
        };
        if self.field_matches(pattern, value)? {
            Ok(())
        } else {
            Err(Error::new(format!("field {name} did not match")))
        }
    }

    fn write_field(&mut self, id: ActorId, name: &str, value: Doc) -> Result<(), Error> {
        let kind = self.instances[&id].kind.clone();
        let fields = self
            .program
            .component(&kind)
            .ok_or_else(|| Error::new("unknown kind"))?
            .fields
            .clone();
        self.match_declared_field(&fields, name, &value)?;
        let projected = project(&value)?;
        let instance = self.instances.get_mut(&id).expect("actor");
        instance.field_json.insert(name.to_string(), projected);
        instance.fields.insert(name.to_string(), value);
        Ok(())
    }

    fn apply_fields(&mut self, id: ActorId, value: Doc) -> Result<(), Error> {
        let kind = self.instances[&id].kind.clone();
        let Some(map) = doc::entries(&value) else {
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

    fn goto_named(&mut self, id: ActorId, name: &str) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
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
        output: Doc,
        next: Option<&Next>,
    ) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
        if let Some(run) = self
            .instances
            .get_mut(&id)
            .and_then(|instance| instance.run.as_mut())
        {
            run.output = output.clone();
            run.output_json = project(&output)?;
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

    fn succeed(&mut self, id: ActorId, output: Doc) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
        {
            let instance = self.instances.get_mut(&id).expect("actor");
            instance.completed = true;
            instance.completion = Some(output.clone());
            instance.completion_json = Some(project(&output)?);
            if let Some(run) = instance.run.as_mut() {
                run.status = RunStatus::Succeeded;
                run.output = output.clone();
                run.output_json = project(&output)?;
            }
        }
        self.finish_to_parent(id, output)
    }

    fn fail(&mut self, id: ActorId, error: Doc) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
        {
            let instance = self.instances.get_mut(&id).expect("actor");
            instance.completed = true;
            instance.completion = Some(error.clone());
            instance.completion_json = Some(project(&error)?);
            if let Some(run) = instance.run.as_mut() {
                run.status = RunStatus::Failed;
                run.output = error.clone();
                run.output_json = project(&error)?;
            }
        }
        Err(Error::new(format!(
            "workflow failed: {}",
            doc::encode(&error).unwrap_or_else(|_| "error".to_string())
        )))
    }

    fn finish_to_parent(
        &mut self,
        id: ActorId,
        output: Doc,
    ) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
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
        message: &Doc,
        route: Route,
    ) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
        if self.instances[&id].driver.is_some() {
            if route == Route::Init {
                return Ok(vec![]);
            }
            return self.drive(id, message);
        }
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
            if let Some(mut bindings) = match_handler(handler, message, &self.program.rules)? {
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
        bindings: &HashMap<String, Doc>,
    ) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
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
                        instance.completion_json = Some(project(&value)?);
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
        payload: Doc,
    ) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
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
        bindings: &HashMap<String, Doc>,
        sends: &mut Vec<(ActorId, Doc, Route)>,
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

    fn fill_join(&mut self, join_id: u64, slot: usize, value: Doc) -> Result<Option<Doc>, Error> {
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
        let values: Vec<Doc> = join
            .slots
            .into_iter()
            .map(|slot| slot.expect("filled join"))
            .collect();
        Ok(Some(if values.len() == 1 {
            values.into_iter().next().expect("one")
        } else {
            doc::array(values)
        }))
    }

    fn eval(&mut self, expression: &Json, bindings: &HashMap<String, Doc>) -> Result<Doc, Error> {
        match self.eval_slot(expression, bindings)? {
            Slot::Ready(value) => Ok(value),
            Slot::Async { .. } => Err(Error::new("async callee is only legal as a send payload")),
        }
    }

    fn eval_slot(
        &mut self,
        expression: &Json,
        bindings: &HashMap<String, Doc>,
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
            return Ok(Slot::Ready(doc::array(values)));
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
                        value: doc::object([("status", doc::number(200.0)), ("url", url)]),
                    });
                }
                name if self.program.functions.iter().any(|item| item == name) => {
                    let mut values = Vec::with_capacity(args.len());
                    for arg in args {
                        values.push(self.eval(arg, bindings)?);
                    }
                    return Ok(Slot::Ready(builtin::call(name, &values)?));
                }
                _ => {}
            }
        }
        let (expression, extras) = self.lower_hosts(expression, bindings)?;
        let mut bindings = bindings.clone();
        bindings.extend(extras);
        Ok(Slot::Ready(evaluate_document(&expression, &bindings)?))
    }

    fn lower_hosts(
        &mut self,
        expression: &Json,
        bindings: &HashMap<String, Doc>,
    ) -> Result<(Json, HashMap<String, Doc>), Error> {
        let mut extras = HashMap::new();
        let mut seq = 0u32;
        let lowered = self.lower_node(expression, bindings, &mut extras, &mut seq)?;
        Ok((lowered, extras))
    }

    fn lower_node(
        &mut self,
        expression: &Json,
        bindings: &HashMap<String, Doc>,
        extras: &mut HashMap<String, Doc>,
        seq: &mut u32,
    ) -> Result<Json, Error> {
        if let Some(name) = invocation_name(expression) {
            if self.program.functions.iter().any(|item| item == name) {
                let value = self.eval(expression, bindings)?;
                let key = format!("$k{seq}");
                *seq += 1;
                extras.insert(key.clone(), value);
                return Ok(json!({"kind": "reference", "name": key}));
            }
        }
        match expression {
            Json::Array(items) => {
                let mut out = Vec::with_capacity(items.len());
                for item in items {
                    out.push(self.lower_node(item, bindings, extras, seq)?);
                }
                Ok(Json::Array(out))
            }
            Json::Object(map) => {
                let mut out = serde_json::Map::new();
                for (key, value) in map {
                    out.insert(key.clone(), self.lower_node(value, bindings, extras, seq)?);
                }
                Ok(Json::Object(out))
            }
            other => Ok(other.clone()),
        }
    }

    fn field_matches(&self, pattern: &Json, value: &Doc) -> Result<bool, Error> {
        if builtin::uses_rule(pattern, &self.program.rules) {
            return Ok(builtin::match_with_rules(pattern, value, &self.program.rules)?.is_some());
        }
        match_field_document(pattern, value)
    }

    fn drive(&mut self, id: ActorId, message: &Doc) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
        let parent = self.instances[&id].parent;
        let driver = self.instances[&id]
            .driver
            .clone()
            .ok_or_else(|| Error::new("missing driver"))?;
        let replies = match driver {
            Driver::Pipe(pipe) => {
                let (pipe, replies) = builtin::drive_pipe(pipe, message);
                self.instances.get_mut(&id).expect("actor").driver = Some(Driver::Pipe(pipe));
                replies
            }
            Driver::Clock => vec![self.clock_reply(message)],
        };
        let Some(parent) = parent else {
            return Ok(vec![]);
        };
        Ok(replies
            .into_iter()
            .map(|reply| (parent, reply, Route::Up))
            .collect())
    }

    fn clock_reply(&mut self, message: &Doc) -> Doc {
        match doc::as_str(message).as_deref() {
            Some("Now") => {
                if self.clock_script.as_ref().is_some_and(Vec::is_empty) {
                    return doc::array([doc::string("Error"), doc::string("clock")]);
                }
                let instant = if let Some(script) = self.clock_script.as_mut() {
                    script.remove(0)
                } else {
                    builtin::instant_doc(builtin::wall_millis())
                };
                self.clock_replies.push(instant.clone());
                doc::array([doc::string("Instant"), instant])
            }
            Some("Zone") => doc::array([doc::string("Zone"), self.clock_zone.clone()]),
            _ => doc::array([doc::string("Error"), doc::string("unknown")]),
        }
    }

    fn eval_write(
        &mut self,
        name: &str,
        args: &[Json],
        bindings: &HashMap<String, Doc>,
    ) -> Result<Doc, Error> {
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
        // `print` records the JSON observation tests read, and writes codec text.
        eprint!("{}", doc::encode(&payload)?);
        self.prints.push(project(&payload)?);
        Ok(doc::null())
    }

    fn eval_payload(
        &mut self,
        args: &[Json],
        bindings: &HashMap<String, Doc>,
    ) -> Result<Doc, Error> {
        match args {
            [] => Ok(doc::null()),
            [one] => self.eval(one, bindings),
            many => {
                let mut values = Vec::with_capacity(many.len());
                for arg in many {
                    values.push(self.eval(arg, bindings)?);
                }
                Ok(doc::array(values))
            }
        }
    }

    fn rand_between(&mut self, lo: &Doc, hi: &Doc) -> Result<Doc, Error> {
        let lo = doc::as_number(lo).ok_or_else(|| Error::new("rand expected numbers"))?;
        let hi = doc::as_number(hi).ok_or_else(|| Error::new("rand expected numbers"))?;
        if lo > hi {
            return Err(Error::new("rand low bound is greater than high bound"));
        }
        self.rng = self.rng.wrapping_mul(1664525).wrapping_add(1013904223);
        if lo.fract() == 0.0 && hi.fract() == 0.0 {
            let width = (hi as i64 - lo as i64 + 1) as u64;
            let pick = lo as i64 + (self.rng % width) as i64;
            return Ok(doc::number(pick as f64));
        }
        let t = (self.rng as f64) / (u64::MAX as f64);
        Ok(doc::number(lo + (hi - lo) * t))
    }

    fn delay_ms(&mut self, after: &Json, bindings: &HashMap<String, Doc>) -> Result<u64, Error> {
        let value = self.eval(after, bindings)?;
        let seconds =
            doc::as_number(&value).ok_or_else(|| Error::new("send after expected seconds"))?;
        if seconds < 0.0 {
            return Err(Error::new("send after delay must not be negative"));
        }
        Ok((seconds * 1000.0) as u64)
    }

    fn deliver(&mut self, to: ActorId, value: Doc, route: Route) -> Result<(), Error> {
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

fn assemble_slots(slots: Vec<Slot>) -> Doc {
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
        doc::array(values)
    }
}

fn stream(name: &str) -> Doc {
    doc::object([("kind", doc::string("stream")), ("name", doc::string(name))])
}

fn bind_world(bindings: &mut HashMap<String, Doc>) {
    for name in ["stdin", "stdout", "stderr"] {
        bindings
            .entry(name.to_string())
            .or_insert_with(|| stream(name));
    }
}

fn check_stream(value: &Doc, name: &str) -> Result<(), Error> {
    if doc::as_str(&doc::get(value, "kind").unwrap_or_else(doc::null)).as_deref() != Some("stream")
    {
        return Err(Error::new(format!("{name} expected a stream")));
    }
    match doc::as_str(&doc::get(value, "name").unwrap_or_else(doc::null)).as_deref() {
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

fn bind_step(
    pattern: &Json,
    working: &Doc,
    rules: &[String],
) -> Result<HashMap<String, Doc>, Error> {
    match_program(pattern, working, rules)?.ok_or_else(|| Error::new("step pattern missed"))
}

fn match_program(
    pattern: &Json,
    message: &Doc,
    rules: &[String],
) -> Result<Option<HashMap<String, Doc>>, Error> {
    if builtin::uses_rule(pattern, rules) {
        builtin::match_with_rules(pattern, message, rules)
    } else {
        match_document(pattern, message)
    }
}

fn match_handler(
    handler: &crate::ast::Handler,
    message: &Doc,
    rules: &[String],
) -> Result<Option<HashMap<String, Doc>>, Error> {
    let Some(item_pattern) = &handler.item else {
        return match_program(&handler.pattern, message, rules);
    };
    let Some(items) = doc::items(message) else {
        return Ok(None);
    };
    let [acc, item] = items.as_slice() else {
        return Ok(None);
    };
    let Some(mut bindings) = match_program(&handler.pattern, acc, rules)? else {
        return Ok(None);
    };
    let Some(item_bindings) = match_program(item_pattern, item, rules)? else {
        return Ok(None);
    };
    bindings.extend(item_bindings);
    Ok(Some(bindings))
}

fn bindings_from(value: &Doc) -> HashMap<String, Doc> {
    doc::entries(value)
        .map(|entries| entries.into_iter().collect())
        .unwrap_or_default()
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
    message: &Doc,
) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
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

fn instance_document(id: u64, instance: &InstanceState) -> Result<Doc, Error> {
    let children = doc::array(instance.children.iter().map(|child| {
        doc::object([
            ("id", doc::number(child.id.0 as f64)),
            (
                "key",
                child.key.clone().map(doc::string).unwrap_or_else(doc::null),
            ),
            ("component", doc::string(&child.component)),
        ])
    }));
    let join = match &instance.join {
        None => doc::null(),
        Some(join) => doc::object([
            (
                "remaining",
                doc::array(
                    join.remaining
                        .iter()
                        .map(|ActorId(id)| doc::number(*id as f64)),
                ),
            ),
            ("total", doc::number(join.total as f64)),
            ("completed", doc::number(join.completed as f64)),
            (
                "reducer",
                join.reducer
                    .map(|ActorId(id)| doc::number(id as f64))
                    .unwrap_or_else(doc::null),
            ),
        ]),
    };
    let run = match &instance.run {
        None => doc::null(),
        Some(run) => run_document(run)?,
    };
    Ok(doc::object([
        ("id", doc::number(id as f64)),
        ("kind", doc::string(&instance.kind)),
        (
            "parent",
            instance
                .parent
                .map(|ActorId(id)| doc::number(id as f64))
                .unwrap_or_else(doc::null),
        ),
        ("children", children),
        (
            "fields",
            doc::object(
                instance
                    .fields
                    .iter()
                    .map(|(name, value)| (name.clone(), value.clone())),
            ),
        ),
        ("state", doc::string(&instance.state)),
        ("completed", doc::boolean(instance.completed)),
        (
            "completion",
            instance.completion.clone().unwrap_or_else(doc::null),
        ),
        ("join", join),
        ("run", run),
        (
            "driver",
            instance
                .driver
                .as_ref()
                .map(builtin::driver_document)
                .unwrap_or_else(doc::null),
        ),
    ]))
}

fn run_document(run: &WorkflowRun) -> Result<Doc, Error> {
    Ok(doc::object([
        (
            "status",
            doc::string(match run.status {
                RunStatus::Running => "running",
                RunStatus::Succeeded => "succeeded",
                RunStatus::Failed => "failed",
            }),
        ),
        ("step", doc::string(&run.step)),
        ("input", run.input.clone()),
        ("output", run.output.clone()),
        // Workflow syntax trees are JSON. They are embedded in the snapshot here.
        ("definition", syntax_document(&run.definition)?),
        ("prototypes", syntax_document(&run.prototypes)?),
    ]))
}

fn syntax_document(value: &impl serde::Serialize) -> Result<Doc, Error> {
    let json = serde_json::to_value(value).map_err(|error| Error::new(error.to_string()))?;
    doc::from_json(&json)
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

fn restore_timer(entry: &Doc) -> Result<Timer, Error> {
    Ok(Timer {
        due: doc::as_u64(&required(entry, "due")?)
            .ok_or_else(|| Error::new("timer missing due"))?,
        to: ActorId(
            doc::as_u64(&required(entry, "to")?).ok_or_else(|| Error::new("timer missing to"))?,
        ),
        message: required(entry, "message")?,
        route: restore_route(
            doc::as_str(&doc::get(entry, "route").unwrap_or_else(doc::null)).as_deref(),
        ),
        join: doc::get(entry, "join")
            .filter(|value| !doc::is_null(value))
            .and_then(|value| doc::as_u64(&value)),
        slot: doc::get(entry, "slot")
            .filter(|value| !doc::is_null(value))
            .and_then(|value| doc::as_u64(&value))
            .map(|slot| slot as usize),
    })
}

fn restore_join(entry: &Doc) -> Result<PendingJoin, Error> {
    let remaining = doc::as_u64(&required(entry, "remaining")?)
        .ok_or_else(|| Error::new("join missing remaining"))? as usize;
    let slots = doc::items(&required(entry, "slots")?)
        .ok_or_else(|| Error::new("join missing slots"))?
        .into_iter()
        .map(|slot| {
            if doc::is_null(&slot) {
                None
            } else {
                Some(slot)
            }
        })
        .collect();
    Ok(PendingJoin {
        remaining,
        slots,
        to: ActorId(
            doc::as_u64(&required(entry, "to")?).ok_or_else(|| Error::new("join missing to"))?,
        ),
        route: restore_route(
            doc::as_str(&doc::get(entry, "route").unwrap_or_else(doc::null)).as_deref(),
        ),
    })
}

fn restore_instance(entry: &Doc) -> Result<InstanceState, Error> {
    let kind = doc::as_str(&required(entry, "kind")?)
        .ok_or_else(|| Error::new("instance missing kind"))?;
    let parent = doc::get(entry, "parent")
        .filter(|value| !doc::is_null(value))
        .and_then(|value| doc::as_u64(&value))
        .map(ActorId);
    let mut children = Vec::new();
    if let Some(list) = doc::get(entry, "children").and_then(|value| doc::items(&value)) {
        for child in list {
            children.push(ChildRef {
                id: ActorId(
                    doc::as_u64(&required(&child, "id")?)
                        .ok_or_else(|| Error::new("child missing id"))?,
                ),
                key: doc::as_str(&doc::get(&child, "key").unwrap_or_else(doc::null)),
                component: doc::as_str(&doc::get(&child, "component").unwrap_or_else(doc::null))
                    .unwrap_or_default(),
            });
        }
    }
    let fields =
        doc::entries(&doc::get(entry, "fields").unwrap_or_else(|| doc::object::<&str>([])))
            .unwrap_or_default()
            .into_iter()
            .collect::<HashMap<_, _>>();
    let field_json = project_map(&fields)?;
    let join = doc::get(entry, "join")
        .filter(|value| !doc::is_null(value))
        .map(|value| {
            let remaining = doc::items(&doc::get(&value, "remaining").unwrap_or_else(doc::null))
                .unwrap_or_default()
                .iter()
                .filter_map(doc::as_u64)
                .map(ActorId)
                .collect();
            JoinSet {
                remaining,
                total: doc::as_u64(&doc::get(&value, "total").unwrap_or_else(doc::null))
                    .unwrap_or(0),
                completed: doc::as_u64(&doc::get(&value, "completed").unwrap_or_else(doc::null))
                    .unwrap_or(0),
                reducer: doc::get(&value, "reducer")
                    .filter(|item| !doc::is_null(item))
                    .and_then(|item| doc::as_u64(&item))
                    .map(ActorId),
            }
        });
    let run = match doc::get(entry, "run") {
        Some(value) if !doc::is_null(&value) => Some(restore_run(&value)?),
        _ => None,
    };
    let completion = doc::get(entry, "completion").filter(|value| !doc::is_null(value));
    let completion_json = completion.as_ref().map(project).transpose()?;
    Ok(InstanceState {
        kind,
        parent,
        children,
        fields,
        field_json,
        state: doc::as_str(&doc::get(entry, "state").unwrap_or_else(doc::null)).unwrap_or_default(),
        completed: doc::as_bool(&doc::get(entry, "completed").unwrap_or_else(doc::null))
            .unwrap_or(false),
        completion,
        completion_json,
        join,
        run,
        driver: doc::get(entry, "driver")
            .filter(|value| !doc::is_null(value))
            .as_ref()
            .and_then(builtin::driver_from),
    })
}

fn restore_run(value: &Doc) -> Result<WorkflowRun, Error> {
    let definition: Workflow =
        serde_json::from_value(doc::to_json(&required(value, "definition")?)?)
            .map_err(|error| Error::new(error.to_string()))?;
    let prototypes = serde_json::from_value(doc::to_json(
        &doc::get(value, "prototypes").unwrap_or_else(|| doc::array([])),
    )?)
    .map_err(|error| Error::new(error.to_string()))?;
    let input = doc::get(value, "input").unwrap_or_else(doc::null);
    let output = doc::get(value, "output").unwrap_or_else(doc::null);
    Ok(WorkflowRun {
        status: match doc::as_str(&doc::get(value, "status").unwrap_or_else(doc::null)).as_deref() {
            Some("succeeded") => RunStatus::Succeeded,
            Some("failed") => RunStatus::Failed,
            _ => RunStatus::Running,
        },
        step: doc::as_str(&doc::get(value, "step").unwrap_or_else(doc::null)).unwrap_or_default(),
        input,
        output: output.clone(),
        output_json: project(&output)?,
        definition,
        prototypes,
    })
}

fn required(doc: &Doc, key: &str) -> Result<Doc, Error> {
    doc::get(doc, key).ok_or_else(|| Error::new(format!("snapshot missing {key}")))
}

fn project(doc: &Doc) -> Result<Json, Error> {
    doc::to_json(doc)
}

fn project_map(fields: &HashMap<String, Doc>) -> Result<HashMap<String, Json>, Error> {
    fields
        .iter()
        .map(|(name, value)| Ok((name.clone(), project(value)?)))
        .collect()
}
