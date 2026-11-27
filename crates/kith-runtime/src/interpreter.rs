use crate::ast::{Component, ConstructArg, Field, Instance, Program, Statement};
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
    /// `send down`: offered to a child. A miss is ignored.
    Offer,
    Up,
    Init,
    SelfSend,
}

#[derive(Clone)]
struct ChildRef {
    id: ActorId,
    key: Option<String>,
    component: String,
    scene: bool,
    held_key: Doc,
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
    driver: Option<Driver>,
}

struct MachineSnap {
    fields: HashMap<String, Doc>,
    field_json: HashMap<String, Json>,
    state: String,
    completed: bool,
    completion: Option<Doc>,
    completion_json: Option<Json>,
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
    crypto_rng: u64,
    argv: Vec<String>,
    env: HashMap<String, String>,
    exit_code: Option<u8>,
    suppress_result: bool,
    hub: IoHub,
    pending_release: Vec<ActorId>,
    pending_init: Vec<ActorId>,
    pending_dynamic: Vec<ActorId>,
}

struct IoHub {
    tx: std::sync::mpsc::Sender<builtin::IoDone>,
    rx: std::sync::mpsc::Receiver<builtin::IoDone>,
    outstanding: usize,
    shared: std::sync::Arc<builtin::IoShared>,
    listening: usize,
}

impl IoHub {
    fn new() -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        Self {
            tx,
            rx,
            outstanding: 0,
            shared: std::sync::Arc::new(builtin::IoShared::new()),
            listening: 0,
        }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        self.hub.shared.stop_all();
    }
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
            check_constructs(program, component)?;
            check_fields(component)?;
            check_field_port_names(component)?;
            check_handler_scope(component)?;
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
            crypto_rng: 0x00c0_ffee,
            argv: Vec::new(),
            env: std::env::vars().collect(),
            exit_code: None,
            suppress_result: false,
            hub: IoHub::new(),
            pending_release: Vec::new(),
            pending_init: Vec::new(),
            pending_dynamic: Vec::new(),
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
        self.instances.get(&id)?.completion_json.as_ref()
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

    /// The key the parent holds for this child. Null when the parent did not
    /// supply one. This is not a runtime-minted identity.
    pub fn held_key(&self, id: ActorId) -> Option<Json> {
        for instance in self.instances.values() {
            if let Some(child) = instance.children.iter().find(|child| child.id == id) {
                return project(&child.held_key).ok();
            }
        }
        None
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
        self.instances.get(&id)?.completion.clone()
    }

    pub fn prints(&self) -> &[Json] {
        &self.prints
    }

    pub fn field_document(&self, id: ActorId, name: &str) -> Option<Doc> {
        self.instances.get(&id)?.fields.get(name).cloned()
    }

    pub fn set_argv(&mut self, argv: Vec<String>) {
        self.argv = argv;
    }

    /// Replace the frozen argv and env. Later messages see this copy.
    pub fn replace_process(&mut self, argv: Vec<String>, env: &[(&str, &str)]) {
        self.argv = argv;
        self.env = env
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect();
    }

    pub fn exit_code(&self) -> Option<u8> {
        self.exit_code
    }

    pub fn suppress_result(&self) -> bool {
        self.suppress_result
    }

    pub fn outstanding(&self) -> usize {
        self.hub.outstanding
    }

    pub fn listening(&self) -> bool {
        self.hub.listening > 0
    }

    /// Install reply batches on an edge driver. An empty batch leaves the
    /// next command in flight.
    pub fn script_driver(&mut self, id: ActorId, batches: Vec<Vec<Doc>>) {
        if let Some(builtin::Driver::Edge(edge)) = self
            .instances
            .get_mut(&id)
            .and_then(|instance| instance.driver.as_mut())
        {
            edge.install_script(batches);
        }
    }

    /// Deliver replies for a command a script left in flight.
    pub fn answer(&mut self, id: ActorId, replies: Vec<Doc>) -> Result<(), Error> {
        if let Some(builtin::Driver::Edge(edge)) = self
            .instances
            .get_mut(&id)
            .and_then(|instance| instance.driver.as_mut())
        {
            edge.clear_busy();
        }
        let Some(parent) = self.instances.get(&id).and_then(|instance| instance.parent) else {
            return Ok(());
        };
        for reply in replies {
            self.deliver(parent, reply, Route::Up)?;
        }
        Ok(())
    }

    pub fn poll_io(&mut self) -> Result<(), Error> {
        loop {
            match self.hub.rx.try_recv() {
                Ok(done) => self.apply_io(done)?,
                Err(std::sync::mpsc::TryRecvError::Empty) => return Ok(()),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => return Ok(()),
            }
        }
    }

    pub fn wait_io(&mut self, timeout: std::time::Duration) -> Result<(), Error> {
        self.poll_io()?;
        if self.hub.outstanding == 0 && self.hub.listening == 0 {
            return Ok(());
        }
        match self.hub.rx.recv_timeout(timeout) {
            Ok(done) => {
                self.apply_io(done)?;
                self.poll_io()
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Ok(()),
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => Ok(()),
        }
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
        self.inject_document(doc::from_json(&message)?)
    }

    pub fn inject_document(&mut self, message: Doc) -> Result<(), Error> {
        self.deliver(self.root_id(), message, Route::Inject)
    }

    pub fn run_until_idle(&mut self) -> Result<(), Error> {
        loop {
            let Some(turn) = self.dispatcher.turn() else {
                return Ok(());
            };
            let route = self.take_route(turn.actor);
            let sends = match self.handle(turn.actor, &turn.message.value, route) {
                Ok(sends) => sends,
                Err(err) => {
                    self.discard_pending_constructs();
                    return Err(err);
                }
            };
            self.apply_releases();
            self.collect_unreachable();
            self.deliver_pending_inits()?;
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
            ("cryptoRng", doc::number(self.crypto_rng as f64)),
            (
                "argv",
                doc::array(self.argv.iter().cloned().map(doc::string)),
            ),
            (
                "env",
                doc::object(
                    self.env
                        .iter()
                        .map(|(key, value)| (key.clone(), doc::string(value))),
                ),
            ),
            (
                "exit",
                self.exit_code
                    .map(|code| doc::number(code as f64))
                    .unwrap_or_else(doc::null),
            ),
            ("suppress", doc::boolean(self.suppress_result)),
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
            crypto_rng: doc::as_u64(&doc::get(&snapshot, "cryptoRng").unwrap_or_else(doc::null))
                .unwrap_or(0x00c0_ffee),
            argv: doc::items(&doc::get(&snapshot, "argv").unwrap_or_else(doc::null))
                .unwrap_or_default()
                .iter()
                .filter_map(|item| doc::as_str(item))
                .collect(),
            env: doc::entries(
                &doc::get(&snapshot, "env").unwrap_or_else(|| doc::object::<&str>([])),
            )
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(key, value)| doc::as_str(&value).map(|value| (key, value)))
            .collect(),
            exit_code: doc::as_u64(&doc::get(&snapshot, "exit").unwrap_or_else(doc::null))
                .and_then(|code| u8::try_from(code).ok()),
            suppress_result: doc::as_bool(
                &doc::get(&snapshot, "suppress").unwrap_or_else(doc::null),
            )
            .unwrap_or(false),
            hub: IoHub::new(),
            pending_release: Vec::new(),
            pending_init: Vec::new(),
            pending_dynamic: Vec::new(),
        })
    }

    fn spawn_instance(
        &mut self,
        node: &Instance,
        parent: Option<ActorId>,
    ) -> Result<ActorId, Error> {
        let id = self.spawn_kind(&node.component, parent, &node.args)?;
        let mut children = Vec::new();
        for child in &node.children {
            let child_id = self.spawn_instance(child, Some(id))?;
            children.push(ChildRef {
                id: child_id,
                key: child.key.clone(),
                component: child.component.clone(),
                scene: true,
                held_key: doc::null(),
            });
        }
        self.instances.get_mut(&id).expect("parent").children = children;
        Ok(id)
    }

    /// The parent holds `node` as a dynamic child. Nested children are
    /// scene-constructed for that child's life. This is not a transition.
    #[doc(hidden)]
    pub fn hold_dynamic_child(
        &mut self,
        parent: ActorId,
        node: &Instance,
    ) -> Result<ActorId, Error> {
        let id = self.spawn_kind(&node.component, Some(parent), &node.args)?;
        let mut children = Vec::new();
        for child in &node.children {
            let child_id = self.spawn_instance(child, Some(id))?;
            children.push(ChildRef {
                id: child_id,
                key: child.key.clone(),
                component: child.component.clone(),
                scene: true,
                held_key: doc::null(),
            });
        }
        self.instances.get_mut(&id).expect("dynamic").children = children;
        self.instances
            .get_mut(&parent)
            .ok_or_else(|| Error::new("unknown parent"))?
            .children
            .push(ChildRef {
                id,
                key: node.key.clone(),
                component: node.component.clone(),
                scene: false,
                held_key: doc::null(),
            });
        Ok(id)
    }

    fn spawn_constructed(
        &mut self,
        parent: ActorId,
        component: &str,
        held_key: Doc,
        children: &[Instance],
    ) -> Result<Vec<ActorId>, Error> {
        check_construct_children(&self.program, component, children)?;
        let id = self.spawn_kind(component, Some(parent), &[])?;
        let mut nested = Vec::new();
        for child in children {
            let child_id = self.spawn_instance(child, Some(id))?;
            nested.push(ChildRef {
                id: child_id,
                key: child.key.clone(),
                component: child.component.clone(),
                scene: true,
                held_key: doc::null(),
            });
        }
        self.instances.get_mut(&id).expect("constructed").children = nested;
        self.instances
            .get_mut(&parent)
            .ok_or_else(|| Error::new("unknown parent"))?
            .children
            .push(ChildRef {
                id,
                key: None,
                component: component.to_string(),
                scene: false,
                held_key,
            });
        let mut ids = vec![id];
        let mut stack = vec![id];
        while let Some(current) = stack.pop() {
            let Some(instance) = self.instances.get(&current) else {
                continue;
            };
            for child in &instance.children {
                ids.push(child.id);
                stack.push(child.id);
            }
        }
        Ok(ids)
    }

    fn discard_pending_constructs(&mut self) {
        let roots: HashSet<ActorId> = self.pending_dynamic.drain(..).collect();
        self.pending_init.clear();
        self.pending_release.clear();
        if roots.is_empty() {
            return;
        }
        for instance in self.instances.values_mut() {
            instance.children.retain(|child| !roots.contains(&child.id));
        }
        self.collect_unreachable();
    }

    fn deliver_pending_inits(&mut self) -> Result<(), Error> {
        let ids = std::mem::take(&mut self.pending_init);
        self.pending_dynamic.clear();
        for id in ids {
            if self.instances.contains_key(&id) {
                self.deliver(id, doc::null(), Route::Init)?;
            }
        }
        Ok(())
    }

    fn enqueue_inits(&mut self) -> Result<(), Error> {
        let mut ids: Vec<_> = self.instances.keys().copied().collect();
        ids.sort_by_key(|id| id.0);
        for id in ids {
            self.deliver(id, doc::null(), Route::Init)?;
        }
        Ok(())
    }

    fn spawn_kind(
        &mut self,
        kind: &str,
        parent: Option<ActorId>,
        args: &[ConstructArg],
    ) -> Result<ActorId, Error> {
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
        if !args.is_empty() && kind != "Timer" {
            return Err(Error::new(format!(
                "{kind} does not take construction arguments"
            )));
        }
        let named = named_args(args)?;
        for name in named.keys() {
            if !component.fields.iter().any(|field| field.name == *name) {
                return Err(Error::new(format!("unknown field {name}")));
            }
        }
        let mut fields = HashMap::new();
        for field in &component.fields {
            if !field_in_state(field, &start) {
                continue;
            }
            let value = if let Some(expression) = named.get(field.name.as_str()) {
                self.eval(expression, &HashMap::new())?
            } else if let Some(init) = &field.init {
                self.eval(init, &HashMap::new())?
            } else {
                doc::null()
            };
            self.match_declared_field(&component.fields, &field.name, &value)?;
            fields.insert(field.name.clone(), value);
        }
        if kind == "Timer" {
            let interval = fields
                .get("interval")
                .ok_or_else(|| Error::new("Timer requires interval"))?;
            timer_delay_ms(interval)?;
        }
        self.match_pre(&component, &start, &fields)?;
        let id = ActorId(self.next_id);
        self.next_id += 1;
        self.dispatcher.register(id);
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
                driver: self.driver_for(kind),
            },
        );
        if kind == "Stdout" {
            self.suppress_result = true;
        }
        if kind == "Timer" {
            self.arm_timer(id)?;
        }
        Ok(id)
    }

    fn arm_timer(&mut self, id: ActorId) -> Result<(), Error> {
        let interval = self.instances[&id]
            .fields
            .get("interval")
            .cloned()
            .ok_or_else(|| Error::new("Timer requires interval"))?;
        let delay = timer_delay_ms(&interval)?;
        self.timers.push(Timer {
            due: self.now.saturating_add(delay),
            to: id,
            message: doc::symbol("Tick"),
            route: Route::SelfSend,
            join: None,
            slot: None,
        });
        Ok(())
    }

    fn driver_for(&self, kind: &str) -> Option<Driver> {
        self.program
            .host_kinds
            .iter()
            .any(|name| name == kind)
            .then(|| builtin::initial_driver(kind))
            .flatten()
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
        let component = self
            .program
            .component(&kind)
            .ok_or_else(|| Error::new("unknown kind"))?;
        let state = self.instances[&id].state.clone();
        let field = component
            .fields
            .iter()
            .find(|field| field.name == name)
            .ok_or_else(|| Error::new(format!("unknown field {name}")))?;
        let Some(clause) = field_clause(field, &state) else {
            return Err(Error::new(format!(
                "field {name} is not in scope in {state}"
            )));
        };
        if !clause.write {
            return Err(Error::new(format!(
                "field {name} is not writable in {state}"
            )));
        }
        self.match_declared_field(&component.fields, name, &value)?;
        let projected = project(&value)?;
        let instance = self.instances.get_mut(&id).expect("actor");
        instance.field_json.insert(name.to_string(), projected);
        instance.fields.insert(name.to_string(), value);
        Ok(())
    }

    fn finish_to_parent(
        &mut self,
        id: ActorId,
        output: Doc,
    ) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
        let Some(parent) = self.instances[&id].parent else {
            return Ok(vec![]);
        };
        Ok(vec![(parent, output, Route::Up)])
    }

    fn handle(
        &mut self,
        id: ActorId,
        message: &Doc,
        route: Route,
    ) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
        self.pending_release.clear();
        self.pending_init.clear();
        self.pending_dynamic.clear();
        if self.instances[&id].driver.is_some() {
            if route == Route::Init {
                return Ok(vec![]);
            }
            return self.drive(id, message, route);
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

    fn machine_snap(&self, id: ActorId) -> MachineSnap {
        let instance = &self.instances[&id];
        MachineSnap {
            fields: instance.fields.clone(),
            field_json: instance.field_json.clone(),
            state: instance.state.clone(),
            completed: instance.completed,
            completion: instance.completion.clone(),
            completion_json: instance.completion_json.clone(),
        }
    }

    fn restore_machine(&mut self, id: ActorId, snap: MachineSnap) {
        let Some(instance) = self.instances.get_mut(&id) else {
            return;
        };
        instance.fields = snap.fields;
        instance.field_json = snap.field_json;
        instance.state = snap.state;
        instance.completed = snap.completed;
        instance.completion = snap.completion;
        instance.completion_json = snap.completion_json;
    }

    /// `pre` of each field that exists in the state being entered.
    fn match_pre(
        &self,
        component: &Component,
        state: &str,
        fields: &HashMap<String, Doc>,
    ) -> Result<(), Error> {
        self.match_field_phase(component, state, fields, true)
    }

    /// `post` of each field that exists in the state being left, after the
    /// leaving handler's assignments and before `goto` commits.
    fn match_post(
        &self,
        component: &Component,
        state: &str,
        fields: &HashMap<String, Doc>,
    ) -> Result<(), Error> {
        self.match_field_phase(component, state, fields, false)
    }

    fn match_field_phase(
        &self,
        component: &Component,
        state: &str,
        fields: &HashMap<String, Doc>,
        pre: bool,
    ) -> Result<(), Error> {
        for field in &component.fields {
            let Some(clause) = field_clause(field, state) else {
                continue;
            };
            let pattern = if pre { &clause.pre } else { &clause.post };
            let value = fields.get(&field.name).cloned().unwrap_or_else(doc::null);
            if !self.field_matches(pattern, &value)? {
                let label = if pre { "pre" } else { "post" };
                return Err(Error::new(format!(
                    "{label} did not match on {} in {state}",
                    component.name
                )));
            }
        }
        Ok(())
    }

    fn propose_scope(
        &mut self,
        component: &Component,
        from: &str,
        to: &str,
        current: &HashMap<String, Doc>,
    ) -> Result<HashMap<String, Doc>, Error> {
        let mut proposed = HashMap::new();
        for field in &component.fields {
            if !field_in_state(field, to) {
                continue;
            }
            if field_in_state(field, from) {
                let value = current.get(&field.name).cloned().unwrap_or_else(doc::null);
                proposed.insert(field.name.clone(), value);
            } else {
                let value = if let Some(init) = &field.init {
                    self.eval(init, &HashMap::new())?
                } else {
                    doc::null()
                };
                self.match_declared_field(&component.fields, &field.name, &value)?;
                proposed.insert(field.name.clone(), value);
            }
        }
        Ok(proposed)
    }

    fn run_statements(
        &mut self,
        id: ActorId,
        statements: &[Statement],
        bindings: &HashMap<String, Doc>,
    ) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
        let snap = self.machine_snap(id);
        let from = snap.state.clone();
        let sends = match self.propose_turn(id, statements, bindings) {
            Ok(sends) => sends,
            Err(err) => {
                self.restore_machine(id, snap);
                return Err(err);
            }
        };
        let (kind_name, state_name, current) = {
            let instance = &self.instances[&id];
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
        let leaving = fields_in_scope(&component, &from, &current);
        let proposed = match self.propose_scope(&component, &from, &state_name, &current) {
            Ok(fields) => fields,
            Err(err) => {
                self.restore_machine(id, snap);
                return Err(err);
            }
        };
        if state_name != from {
            if let Err(err) = self.match_pre(&component, &state_name, &proposed) {
                self.restore_machine(id, snap);
                return Err(err);
            }
            if let Err(err) = self.match_post(&component, &from, &leaving) {
                self.restore_machine(id, snap);
                return Err(err);
            }
        }
        let field_json = project_map(&proposed)?;
        let instance = self.instances.get_mut(&id).expect("actor");
        instance.fields = proposed;
        instance.field_json = field_json;
        Ok(sends)
    }

    fn propose_turn(
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
                    let child_id = self.instances[&id]
                        .children
                        .iter()
                        .find(|child| child.key.as_deref() == Some(port.as_str()))
                        .map(|child| child.id);
                    if let Some(child_id) = child_id {
                        self.enqueue_send(
                            child_id,
                            message,
                            after.as_ref(),
                            Route::Down,
                            &bindings,
                            &mut sends,
                        )?;
                    } else if self.instances[&id].fields.contains_key(port) {
                        let (targets, route) = self.field_targets(id, port)?;
                        for target in targets {
                            self.enqueue_send(
                                target,
                                message,
                                after.as_ref(),
                                route,
                                &bindings,
                                &mut sends,
                            )?;
                        }
                    } else {
                        return Err(Error::new(format!("unknown port {port}")));
                    }
                }
                Statement::SendDown {
                    component,
                    message,
                    after,
                } => {
                    if component.is_some() {
                        return Err(Error::new("send down must not name a component"));
                    }
                    for child_id in self.select_children(id) {
                        self.enqueue_send(
                            child_id,
                            message,
                            after.as_ref(),
                            Route::Offer,
                            &bindings,
                            &mut sends,
                        )?;
                    }
                }
                Statement::Release { filter } => {
                    self.record_release(id, filter)?;
                }
                Statement::Construct {
                    component,
                    key,
                    children,
                } => {
                    let held_key = match key {
                        Some(key) => self.eval(key, &bindings)?,
                        None => doc::null(),
                    };
                    let created = self.spawn_constructed(id, component, held_key, children)?;
                    self.pending_dynamic.push(created[0]);
                    self.pending_init.extend(created);
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

    fn select_children(&self, id: ActorId) -> Vec<ActorId> {
        self.instances
            .get(&id)
            .map(|instance| instance.children.iter().map(|child| child.id).collect())
            .unwrap_or_default()
    }

    fn holds_match(&self, port: Option<&str>, key: &Doc, filter: &Json) -> Result<bool, Error> {
        let record = doc::object([
            ("port", port.map(doc::string).unwrap_or_else(doc::null)),
            ("key", key.clone()),
        ]);
        Ok(match_program(filter, &record, &self.program.rules)?.is_some())
    }

    fn record_release(&mut self, id: ActorId, filter: &Json) -> Result<(), Error> {
        let children = self.instances[&id].children.clone();
        let mut hits = Vec::new();
        for child in &children {
            if !self.holds_match(child.key.as_deref(), &child.held_key, filter)? {
                continue;
            }
            if child.scene {
                return Err(Error::new("release cannot drop a scene-constructed child"));
            }
            hits.push(child.id);
        }
        if hits.is_empty() {
            return Err(Error::new("no child matched release"));
        }
        self.pending_release.extend(hits);
        Ok(())
    }

    fn apply_releases(&mut self) {
        let released: HashSet<ActorId> = self.pending_release.drain(..).collect();
        if released.is_empty() {
            return;
        }
        for instance in self.instances.values_mut() {
            instance
                .children
                .retain(|child| !released.contains(&child.id));
        }
    }

    fn collect_unreachable(&mut self) {
        let mut reachable = HashSet::new();
        let mut stack: Vec<ActorId> = self
            .instances
            .iter()
            .filter(|(_, instance)| instance.parent.is_none())
            .map(|(id, _)| *id)
            .collect();
        while let Some(id) = stack.pop() {
            if !reachable.insert(id) {
                continue;
            }
            if let Some(instance) = self.instances.get(&id) {
                let mut extra = Vec::new();
                for child in &instance.children {
                    extra.push(child.id);
                }
                for value in instance.fields.values() {
                    collect_instance_refs(value, &mut extra);
                }
                stack.extend(extra);
            }
        }
        let dead: Vec<ActorId> = self
            .instances
            .keys()
            .copied()
            .filter(|id| !reachable.contains(id))
            .collect();
        if dead.is_empty() {
            return;
        }
        let dead_set: HashSet<ActorId> = dead.into_iter().collect();
        self.timers.retain(|timer| !dead_set.contains(&timer.to));
        self.joins.retain(|_, join| !dead_set.contains(&join.to));
        let join_ids: HashSet<u64> = self.joins.keys().copied().collect();
        self.timers
            .retain(|timer| timer.join.is_none_or(|id| join_ids.contains(&id)));
        self.routes
            .retain(|(actor, _), _| !dead_set.contains(actor));
        for id in &dead_set {
            self.dispatcher.drop_actor(*id);
            self.instances.remove(id);
        }
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
        if let Some(kind) = component_array_kind(pattern) {
            if self.program.component(kind).is_some() {
                return Ok(self.instance_array_matches(kind, value));
            }
        }
        if builtin::uses_rule(pattern, &self.program.rules) {
            return Ok(builtin::match_with_rules(pattern, value, &self.program.rules)?.is_some());
        }
        match_field_document(pattern, value)
    }

    fn instance_array_matches(&self, kind: &str, value: &Doc) -> bool {
        let Some(items) = doc::items(value) else {
            return false;
        };
        items.iter().all(|item| {
            instance_id(item).is_some_and(|id| {
                self.instances
                    .get(&id)
                    .is_some_and(|instance| instance.kind == kind)
            })
        })
    }

    /// Store `kinds` in `field` as instances. They are field values: not
    /// children, and not a port fill. Each is parented to `id`.
    #[doc(hidden)]
    pub fn hold_field_instances(
        &mut self,
        id: ActorId,
        field: &str,
        kinds: &[&str],
    ) -> Result<Vec<ActorId>, Error> {
        if !self
            .instances
            .get(&id)
            .is_some_and(|instance| instance.fields.contains_key(field))
        {
            return Err(Error::new(format!("unknown field {field}")));
        }
        let mut created = Vec::with_capacity(kinds.len());
        for kind in kinds {
            created.push(self.spawn_kind(kind, Some(id), &[])?);
        }
        let refs = doc::array(created.iter().copied().map(instance_ref));
        let projected = project(&refs)?;
        let instance = self.instances.get_mut(&id).expect("holder");
        instance.fields.insert(field.to_string(), refs);
        instance.field_json.insert(field.to_string(), projected);
        for child in &created {
            self.deliver(*child, doc::null(), Route::Init)?;
        }
        Ok(created)
    }

    /// Store one instance in `field`. It is a field value: not a child,
    /// and not a port fill. It is parented to `id`.
    #[doc(hidden)]
    pub fn hold_field_instance(
        &mut self,
        id: ActorId,
        field: &str,
        kind: &str,
    ) -> Result<ActorId, Error> {
        if !self
            .instances
            .get(&id)
            .is_some_and(|instance| instance.fields.contains_key(field))
        {
            return Err(Error::new(format!("unknown field {field}")));
        }
        let created = self.spawn_kind(kind, Some(id), &[])?;
        let value = instance_ref(created);
        let projected = project(&value)?;
        let instance = self.instances.get_mut(&id).expect("holder");
        instance.fields.insert(field.to_string(), value);
        instance.field_json.insert(field.to_string(), projected);
        self.deliver(created, doc::null(), Route::Init)?;
        Ok(created)
    }

    /// An array value is an offer to each element. Any other value is one
    /// send to that instance. A non-instance fails either way.
    fn field_targets(&self, id: ActorId, name: &str) -> Result<(Vec<ActorId>, Route), Error> {
        let value = self.instances[&id]
            .fields
            .get(name)
            .ok_or_else(|| Error::new(format!("unknown field {name}")))?;
        if let Some(items) = doc::items(value) {
            let mut targets = Vec::with_capacity(items.len());
            for item in items {
                let Some(target) = instance_id(&item) else {
                    return Err(Error::new(format!(
                        "send to {name} element is not an instance"
                    )));
                };
                targets.push(target);
            }
            Ok((targets, Route::Offer))
        } else {
            let Some(target) = instance_id(value) else {
                return Err(Error::new(format!("send to {name} is not an instance")));
            };
            Ok((vec![target], Route::Down))
        }
    }

    fn drive(
        &mut self,
        id: ActorId,
        message: &Doc,
        route: Route,
    ) -> Result<Vec<(ActorId, Doc, Route)>, Error> {
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
            Driver::Timer => {
                if route == Route::SelfSend && doc::as_symbol(message).as_deref() == Some("Tick") {
                    self.arm_timer(id)?;
                    vec![doc::symbol("Tick")]
                } else {
                    vec![doc::array([doc::string("Error"), doc::string("unknown")])]
                }
            }
            Driver::Edge(driver) => {
                let symbols = driver.speaks_symbols();
                let replies = self.drive_edge(id, driver, message)?;
                if symbols {
                    replies.into_iter().map(symbolize_reply).collect()
                } else {
                    replies
                }
            }
        };
        let Some(parent) = parent else {
            return Ok(vec![]);
        };
        Ok(replies
            .into_iter()
            .map(|reply| (parent, reply, Route::Up))
            .collect())
    }

    fn drive_edge(
        &mut self,
        id: ActorId,
        driver: builtin::Edge,
        message: &Doc,
    ) -> Result<Vec<Doc>, Error> {
        let argv = self.argv.clone();
        let env = self.env.clone();
        let shared = std::sync::Arc::clone(&self.hub.shared);
        let mut rng = self.crypto_rng;
        let mut host = builtin::Host {
            argv: &argv,
            env: &env,
            rng: &mut rng,
        };
        let mut outcome = builtin::step(driver, id.0, message, &mut host, &shared);
        self.crypto_rng = rng;
        if let Some(code) = outcome.exit {
            self.exit_code = Some(code);
        }
        if let Some(bytes) = outcome.stdout.take() {
            let mut out = std::io::stdout().lock();
            let _ = std::io::Write::write_all(&mut out, &bytes);
            let _ = std::io::Write::flush(&mut out);
        }
        if let Some(bytes) = outcome.stderr.take() {
            let mut err = std::io::stderr().lock();
            let _ = std::io::Write::write_all(&mut err, &bytes);
            let _ = std::io::Write::flush(&mut err);
        }
        let response = match &outcome.job {
            Some(builtin::Job::WriteResponse { id }) => outcome.edge.take_response(id),
            _ => None,
        };
        if let Some(listen) = outcome.listen.take() {
            match builtin::begin_listen(
                &listen.host,
                listen.port,
                listen.server,
                id.0,
                self.hub.tx.clone(),
                &self.hub.shared,
            ) {
                Ok(port) => {
                    self.hub.listening += 1;
                    outcome.replies.push(doc::array([
                        doc::string("Listening"),
                        doc::string(&listen.host),
                        doc::number(port as f64),
                    ]));
                }
                Err(reason) => {
                    outcome.edge.set_listening(false);
                    outcome
                        .replies
                        .push(doc::array([doc::string("Error"), doc::string(reason)]));
                }
            }
        }
        if outcome.stop_listen {
            builtin::stop_listen(id.0, &self.hub.shared);
            self.hub.listening = self.hub.listening.saturating_sub(1);
        }
        if let Some(job) = outcome.job.take() {
            self.hub.outstanding += 1;
            builtin::spawn_job(
                job,
                id.0,
                self.hub.tx.clone(),
                std::sync::Arc::clone(&self.hub.shared),
                response,
            );
        }
        self.instances.get_mut(&id).expect("actor").driver = Some(Driver::Edge(outcome.edge));
        Ok(outcome.replies)
    }

    fn apply_io(&mut self, mut done: builtin::IoDone) -> Result<(), Error> {
        let actor = ActorId(done.actor);
        if done.track {
            self.hub.outstanding = self.hub.outstanding.saturating_sub(1);
        }
        if let Some(stream) = done.sock.take() {
            self.hub.shared.insert_sock(done.actor, stream);
        }
        if let Some(body) = done.body.take() {
            self.hub.shared.insert_body(done.actor, body);
            if let Some(Driver::Edge(edge)) = self
                .instances
                .get_mut(&actor)
                .and_then(|instance| instance.driver.as_mut())
            {
                edge.open_body();
            }
        }
        if let Some(stream) = done.accepted.take() {
            let id = self.alloc_edge_id(actor, "c");
            self.hub.shared.insert_conn(done.actor, id.clone(), stream);
            done.replies
                .push(doc::array([doc::string("Connection"), doc::string(id)]));
        }
        if let Some(incoming) = done.request.take() {
            let id = self.alloc_edge_id(actor, "r");
            let method = incoming.method;
            let path = incoming.path;
            let headers = doc::object(
                incoming
                    .headers
                    .iter()
                    .map(|(name, value)| (name.clone(), doc::string(value))),
            );
            self.hub
                .shared
                .insert_conn(done.actor, id.clone(), incoming.stream);
            if method.eq_ignore_ascii_case("GET") {
                if let Some(Driver::Edge(edge)) = self
                    .instances
                    .get_mut(&actor)
                    .and_then(|instance| instance.driver.as_mut())
                {
                    edge.note_get(id.clone());
                }
            }
            done.replies.push(doc::array([
                doc::string("Request"),
                doc::string(&id),
                doc::string(method),
                doc::string(path),
                headers,
            ]));
        }
        if done.clear_busy {
            if let Some(Driver::Edge(edge)) = self
                .instances
                .get_mut(&actor)
                .and_then(|instance| instance.driver.as_mut())
            {
                edge.clear_one(done.busy_id.as_deref());
            }
        }
        let Some(parent) = self
            .instances
            .get(&actor)
            .and_then(|instance| instance.parent)
        else {
            return Ok(());
        };
        let symbols = self
            .instances
            .get(&actor)
            .and_then(|instance| instance.driver.as_ref())
            .is_some_and(|driver| match driver {
                Driver::Edge(edge) => edge.speaks_symbols(),
                _ => false,
            });
        for reply in done.replies {
            let reply = if symbols {
                symbolize_reply(reply)
            } else {
                reply
            };
            self.deliver(parent, reply, Route::Up)?;
        }
        Ok(())
    }

    fn alloc_edge_id(&mut self, actor: ActorId, prefix: &str) -> String {
        match self
            .instances
            .get_mut(&actor)
            .and_then(|instance| instance.driver.as_mut())
        {
            Some(Driver::Edge(edge)) => edge.next_id(prefix),
            _ => format!("{prefix}0"),
        }
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
        if !self.instances.contains_key(&to) {
            return Ok(());
        }
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
        "down" => matches!(route, Route::Down | Route::Offer | Route::Inject),
        _ => false,
    }
}

fn timer_delay_ms(interval: &Doc) -> Result<u64, Error> {
    let millis = doc::as_number(interval)
        .filter(|number| number.is_finite())
        .ok_or_else(|| Error::new("Timer interval must be milliseconds"))?;
    if millis <= 0.0 {
        return Err(Error::new("Timer interval must be positive"));
    }
    let seconds = millis / 1000.0;
    Ok((seconds * 1000.0) as u64)
}

fn named_args(args: &[ConstructArg]) -> Result<HashMap<&str, &Json>, Error> {
    let mut named = HashMap::new();
    for arg in args {
        if named.insert(arg.name.as_str(), &arg.expression).is_some() {
            return Err(Error::new(format!(
                "duplicate construction argument {}",
                arg.name
            )));
        }
    }
    Ok(named)
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
                    | Statement::SendTo { message, after, .. } => {
                        let delayed = after.is_some();
                        for item in send_items(message) {
                            check_send_arg(item, delayed, &component.name)?;
                        }
                    }
                    Statement::SendDown {
                        component: selector,
                        message,
                        after,
                    } => {
                        if selector.is_some() {
                            return Err(Error::new(format!(
                                "send down must not name a component in {}",
                                component.name
                            )));
                        }
                        let delayed = after.is_some();
                        for item in send_items(message) {
                            check_send_arg(item, delayed, &component.name)?;
                        }
                    }
                    Statement::Goto { .. }
                    | Statement::Release { .. }
                    | Statement::Construct { .. } => {}
                }
            }
        }
    }
    Ok(())
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
        Route::Offer => Ok(vec![]),
        Route::Inject => Err(Error::new("unmatched injected message")),
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

fn fields_in_scope(
    component: &Component,
    state: &str,
    current: &HashMap<String, Doc>,
) -> HashMap<String, Doc> {
    let mut fields = HashMap::new();
    for field in &component.fields {
        if !field_in_state(field, state) {
            continue;
        }
        if let Some(value) = current.get(&field.name) {
            fields.insert(field.name.clone(), value.clone());
        }
    }
    fields
}

struct FieldClause {
    write: bool,
    pre: Json,
    post: Json,
}

fn field_pattern(field: &Field) -> Json {
    field
        .pattern
        .clone()
        .unwrap_or_else(|| json!({"kind": "any"}))
}

/// `None` rules: every state, writable, `pre` and `post` are the field pattern.
/// The first `in` opts the field into only the named states. A clause's omitted
/// `pre` is the field pattern. An omitted `post` is that `pre`, or the field
/// pattern when `pre` is omitted too. `write` on any clause limits assignment
/// to the clauses that say `write`.
fn field_clause(field: &Field, state: &str) -> Option<FieldClause> {
    let pattern = field_pattern(field);
    let Some(rules) = &field.rules else {
        return Some(FieldClause {
            write: true,
            pre: pattern.clone(),
            post: pattern,
        });
    };
    let any_write = rules.iter().any(|rule| rule.write);
    let rule = rules
        .iter()
        .find(|rule| rule.states.iter().any(|name| name == state))?;
    let pre = rule.pre.clone().unwrap_or_else(|| pattern.clone());
    let post = rule
        .post
        .clone()
        .or_else(|| rule.pre.clone())
        .unwrap_or(pattern);
    Some(FieldClause {
        write: if any_write { rule.write } else { true },
        pre,
        post,
    })
}

fn field_in_state(field: &Field, state: &str) -> bool {
    field_clause(field, state).is_some()
}

fn check_field_port_names(component: &Component) -> Result<(), Error> {
    let fields: HashSet<&str> = component
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    for port in &component.ports {
        if fields.contains(port.name.as_str()) {
            return Err(Error::new(format!(
                "{} is both a field and a port on {}",
                port.name, component.name
            )));
        }
    }
    Ok(())
}

fn check_fields(component: &Component) -> Result<(), Error> {
    let state_names: HashSet<&str> = component
        .states
        .iter()
        .map(|state| state.name.as_str())
        .collect();
    let mut seen = HashSet::new();
    for field in &component.fields {
        if !seen.insert(field.name.as_str()) {
            return Err(Error::new(format!(
                "duplicate field {} on {}",
                field.name, component.name
            )));
        }
        let Some(rules) = &field.rules else {
            continue;
        };
        if rules.is_empty() {
            return Err(Error::new(format!(
                "field {} on {} names no states",
                field.name, component.name
            )));
        }
        let mut named = HashSet::new();
        for rule in rules {
            if rule.states.is_empty() {
                return Err(Error::new(format!(
                    "field {} on {} names no states",
                    field.name, component.name
                )));
            }
            for state in &rule.states {
                if !state_names.contains(state.as_str()) {
                    return Err(Error::new(format!(
                        "field {} on {} names unknown state {state}",
                        field.name, component.name
                    )));
                }
                if !named.insert(state.as_str()) {
                    return Err(Error::new(format!(
                        "field {} on {} names {state} twice",
                        field.name, component.name
                    )));
                }
            }
        }
    }
    Ok(())
}

fn check_handler_scope(component: &Component) -> Result<(), Error> {
    for state in &component.states {
        for handler in &state.handlers {
            let mut writes = Vec::new();
            let mut reads = Vec::new();
            for statement in &handler.body {
                collect_statement_uses(statement, &mut writes, &mut reads);
            }
            for name in &writes {
                let Some(field) = component.fields.iter().find(|field| field.name == *name) else {
                    continue;
                };
                let Some(clause) = field_clause(field, &state.name) else {
                    return Err(Error::new(format!(
                        "field {name} is not in scope in {} on {}",
                        state.name, component.name
                    )));
                };
                if !clause.write {
                    return Err(Error::new(format!(
                        "field {name} is not writable in {} on {}",
                        state.name, component.name
                    )));
                }
            }
            check_names_in_scope(component, &state.name, &reads)?;
        }
    }
    Ok(())
}

fn check_names_in_scope(component: &Component, state: &str, names: &[String]) -> Result<(), Error> {
    for name in names {
        let Some(field) = component.fields.iter().find(|field| field.name == *name) else {
            continue;
        };
        if !field_in_state(field, state) {
            return Err(Error::new(format!(
                "field {name} is not in scope in {state} on {}",
                component.name
            )));
        }
    }
    Ok(())
}

fn collect_statement_uses(
    statement: &Statement,
    writes: &mut Vec<String>,
    reads: &mut Vec<String>,
) {
    match statement {
        Statement::Assign { field, expression } => {
            writes.push(field.clone());
            collect_json_names(expression, reads);
        }
        Statement::Expr { expression } => collect_json_names(expression, reads),
        Statement::SendUp { message, after }
        | Statement::Send { message, after }
        | Statement::SendTo { message, after, .. }
        | Statement::SendDown { message, after, .. } => {
            collect_json_names(message, reads);
            if let Some(after) = after {
                collect_json_names(after, reads);
            }
        }
        Statement::Construct { key, .. } => {
            if let Some(key) = key {
                collect_json_names(key, reads);
            }
        }
        Statement::Complete { message } => collect_json_names(message, reads),
        Statement::Release { .. } | Statement::Goto { .. } => {}
    }
}

fn collect_json_names(value: &Json, names: &mut Vec<String>) {
    match value {
        Json::Array(items) => {
            for item in items {
                collect_json_names(item, names);
            }
        }
        Json::Object(map) => {
            if map.get("kind").and_then(Json::as_str) == Some("reference") {
                if let Some(name) = map.get("name").and_then(Json::as_str) {
                    names.push(name.to_string());
                }
            }
            for child in map.values() {
                collect_json_names(child, names);
            }
        }
        _ => {}
    }
}

fn check_constructs(program: &Program, component: &crate::ast::Component) -> Result<(), Error> {
    for state in &component.states {
        for handler in &state.handlers {
            for statement in &handler.body {
                if let Statement::Construct {
                    component: kind,
                    children,
                    key,
                } = statement
                {
                    if let Some(key) = key {
                        check_sync(key, &component.name)?;
                    }
                    check_construct_children(program, kind, children)?;
                }
            }
        }
    }
    Ok(())
}

fn check_construct_children(
    program: &Program,
    component: &str,
    children: &[Instance],
) -> Result<(), Error> {
    let kind = program
        .component(component)
        .ok_or_else(|| Error::new(format!("unknown kind {component}")))?;
    let ports: Vec<(String, String)> = kind
        .ports
        .iter()
        .map(|port| (port.name.clone(), port.component.clone()))
        .collect();
    if !ports.is_empty() {
        let mut filled = HashSet::new();
        for child in children {
            if let Some(key) = child.key.as_deref() {
                if !filled.insert(key.to_string()) {
                    return Err(Error::new(format!("duplicate port fill {key}")));
                }
                match ports.iter().find(|(name, _)| name == key) {
                    None => return Err(Error::new(format!("unknown port {key}"))),
                    Some((_, expected)) if expected != &child.component => {
                        return Err(Error::new(format!(
                            "port {key} expects {expected} but was filled with {}",
                            child.component
                        )));
                    }
                    Some(_) => {}
                }
            }
            check_construct_children(program, &child.component, &child.children)?;
        }
        for (name, _) in &ports {
            if !filled.contains(name) {
                return Err(Error::new(format!("missing port fill {name}")));
            }
        }
    } else {
        for child in children {
            check_construct_children(program, &child.component, &child.children)?;
        }
    }
    Ok(())
}

fn check_instance(program: &Program, node: &Instance) -> Result<(), Error> {
    let ports: Vec<(String, String)> = {
        let kind = program
            .kind(&node.component)
            .ok_or_else(|| Error::new(format!("unknown kind {}", node.component)))?;
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
            ("scene", doc::boolean(child.scene)),
            ("heldKey", child.held_key.clone()),
        ])
    }));
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

fn snapshot_route(route: Route) -> &'static str {
    match route {
        Route::Inject => "inject",
        Route::Down => "down",
        Route::Offer => "offer",
        Route::Up => "up",
        Route::Init => "init",
        Route::SelfSend => "self",
    }
}

fn restore_route(value: Option<&str>) -> Route {
    match value {
        Some("down") => Route::Down,
        Some("offer") => Route::Offer,
        Some("up") => Route::Up,
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
                scene: doc::as_bool(&doc::get(&child, "scene").unwrap_or_else(doc::null))
                    .unwrap_or(true),
                held_key: doc::get(&child, "heldKey").unwrap_or_else(doc::null),
            });
        }
    }
    let fields =
        doc::entries(&doc::get(entry, "fields").unwrap_or_else(|| doc::object::<&str>([])))
            .unwrap_or_default()
            .into_iter()
            .collect::<HashMap<_, _>>();
    let field_json = project_map(&fields)?;
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
        driver: doc::get(entry, "driver")
            .filter(|value| !doc::is_null(value))
            .as_ref()
            .and_then(builtin::driver_from),
    })
}

fn required(doc: &Doc, key: &str) -> Result<Doc, Error> {
    doc::get(doc, key).ok_or_else(|| Error::new(format!("snapshot missing {key}")))
}

/// Http and process drivers build string heads. Callers of those drivers
/// speak registry symbols, so the head is a symbol at delivery.
fn symbolize_reply(message: Doc) -> Doc {
    if let Some(name) = doc::as_str(&message) {
        return doc::symbol(name);
    }
    if let Some(mut items) = doc::items(&message) {
        if let Some(name) = items.first().and_then(doc::as_str) {
            items[0] = doc::symbol(name);
            return doc::array(items);
        }
    }
    message
}

fn instance_ref(id: ActorId) -> Doc {
    doc::object([
        ("kind", doc::symbol("kith.instance")),
        ("id", doc::number(id.0 as f64)),
    ])
}

fn instance_id(value: &Doc) -> Option<ActorId> {
    let kind = doc::as_symbol(&doc::get(value, "kind")?)?;
    if kind != "kith.instance" {
        return None;
    }
    Some(ActorId(doc::as_u64(&doc::get(value, "id")?)?))
}

fn collect_instance_refs(value: &Doc, into: &mut Vec<ActorId>) {
    if let Some(id) = instance_id(value) {
        into.push(id);
        return;
    }
    if let Some(items) = doc::items(value) {
        for item in items {
            collect_instance_refs(&item, into);
        }
        return;
    }
    if let Some(entries) = doc::entries(value) {
        for (_, child) in entries {
            collect_instance_refs(&child, into);
        }
    }
}

/// `[Tree*]` is an `into` of a zero-or-more quantifier of a resolve named
/// `Tree`. Single or/and/then/pipeline wrappers are peeled. `Some` only when
/// the quantifier allows zero.
fn component_array_kind(pattern: &Json) -> Option<&str> {
    let pattern = peel_pattern(pattern);
    if pattern.get("kind").and_then(Json::as_str) != Some("into") {
        return None;
    }
    let quantifier = peel_pattern(pattern.get("pattern")?);
    if quantifier.get("kind").and_then(Json::as_str) != Some("quantifier") {
        return None;
    }
    if quantifier_min(quantifier) != 0 {
        return None;
    }
    let element = peel_pattern(quantifier.get("pattern")?);
    if element.get("kind").and_then(Json::as_str) != Some("resolve") {
        return None;
    }
    element.get("name").and_then(Json::as_str)
}

fn peel_pattern(pattern: &Json) -> &Json {
    let mut current = pattern;
    loop {
        let kind = current.get("kind").and_then(Json::as_str);
        let next = match kind {
            Some("or" | "and" | "then") => current
                .get("patterns")
                .and_then(Json::as_array)
                .filter(|items| items.len() == 1)
                .map(|items| &items[0]),
            Some("pipeline") => current
                .get("steps")
                .and_then(Json::as_array)
                .filter(|items| items.len() == 1)
                .map(|items| &items[0]),
            Some("group") => current.get("pattern"),
            _ => None,
        };
        match next {
            Some(next) => current = next,
            None => return current,
        }
    }
}

fn quantifier_min(pattern: &Json) -> i64 {
    match pattern.get("min") {
        None | Some(Json::Null) => 0,
        Some(Json::Number(value)) => value.as_i64().unwrap_or(1),
        Some(Json::Object(map)) => map
            .get("value")
            .and_then(Json::as_i64)
            .or_else(|| {
                map.get("value")
                    .and_then(Json::as_f64)
                    .map(|value| value as i64)
            })
            .unwrap_or(0),
        Some(_) => 1,
    }
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
