use crate::ast::{Instance, Program, Statement};
use crate::dispatch::{ActorId, Dispatcher, Message};
use crate::error::Error;
use crate::host::{evaluate, match_message};
use serde_json::Value as Json;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    Inject,
    Down,
    Up,
}

struct ChildRef {
    id: ActorId,
    key: Option<String>,
    component: String,
}

struct InstanceState {
    kind: String,
    parent: Option<ActorId>,
    children: Vec<ChildRef>,
    fields: HashMap<String, Json>,
    state: String,
}

pub struct World {
    program: Program,
    instances: HashMap<ActorId, InstanceState>,
    next_id: u64,
    dispatcher: Dispatcher,
    routes: HashMap<(ActorId, u64), Route>,
    route_seq: u64,
}

impl World {
    pub fn check(program: &Program) -> Result<(), Error> {
        if program.kind != "program" {
            return Err(Error::new("expected a program tree"));
        }
        let scene = program.unique_exported_scene().map_err(Error::new)?;
        for component in &program.components {
            if component.start_state().is_none() {
                return Err(Error::new(format!(
                    "component {} must declare exactly one start state",
                    component.name
                )));
            }
            let mut port_names = HashSet::new();
            for port in &component.ports {
                if !port_names.insert(port.name.as_str()) {
                    return Err(Error::new(format!("duplicate port {}", port.name)));
                }
                if program.component(&port.component).is_none() {
                    return Err(Error::new(format!(
                        "unknown port kind {}",
                        port.component
                    )));
                }
            }
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

    pub fn root_fields(&self) -> HashMap<String, Json> {
        self.instances
            .get(&self.root_id())
            .map(|instance| instance.fields.clone())
            .unwrap_or_default()
    }

    pub fn inject(&mut self, message: Json) -> Result<(), Error> {
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

    fn spawn_instance(
        &mut self,
        node: &Instance,
        parent: Option<ActorId>,
    ) -> Result<ActorId, Error> {
        let kind = node.component.clone();
        let component = self
            .program
            .component(&kind)
            .ok_or_else(|| Error::new(format!("unknown component {kind}")))?
            .clone();
        let start = component
            .start_state()
            .ok_or_else(|| Error::new(format!("component {kind} has no start state")))?;
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
                kind,
                parent,
                children: Vec::new(),
                fields,
                state: start.name.clone(),
            },
        );
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

    fn handle(
        &mut self,
        id: ActorId,
        message: &Json,
        route: Route,
    ) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        let (component, state_name, fields) = {
            let instance = self
                .instances
                .get(&id)
                .ok_or_else(|| Error::new("unknown actor"))?;
            (
                self.program
                    .component(&instance.kind)
                    .ok_or_else(|| Error::new("unknown kind"))?
                    .clone(),
                instance.state.clone(),
                instance.fields.clone(),
            )
        };
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
                return self.run_body(id, handler, &bindings);
            }
        }
        match route {
            Route::Up => {
                if let Some(parent) = self.instances[&id].parent {
                    Ok(vec![(parent, message.clone(), Route::Up)])
                } else {
                    Ok(vec![])
                }
            }
            Route::Down => Err(Error::new(format!(
                "unmatched down to {}",
                self.instances[&id].kind
            ))),
            Route::Inject => Err(Error::new("unmatched injected message")),
        }
    }

    fn run_body(
        &mut self,
        id: ActorId,
        handler: &crate::ast::Handler,
        bindings: &HashMap<String, Json>,
    ) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        let mut sends = Vec::new();
        for statement in &handler.body {
            match statement {
                Statement::Assign { field, expression } => {
                    let value = evaluate(expression, bindings)?;
                    self.instances
                        .get_mut(&id)
                        .expect("actor")
                        .fields
                        .insert(field.clone(), value);
                }
                Statement::SendUp { message } => {
                    let value = evaluate(message, bindings)?;
                    let parent = self.instances[&id]
                        .parent
                        .ok_or_else(|| Error::new("send up from root"))?;
                    sends.push((parent, value, Route::Up));
                }
                Statement::SendTo { port, message } => {
                    let value = evaluate(message, bindings)?;
                    let child = self.instances[&id]
                        .children
                        .iter()
                        .find(|child| child.key.as_deref() == Some(port.as_str()))
                        .ok_or_else(|| Error::new(format!("unknown port {port}")))?;
                    sends.push((child.id, value, Route::Down));
                }
                Statement::SendDown {
                    component,
                    message,
                } => {
                    let value = evaluate(message, bindings)?;
                    let selected = self.select_children(id, component.as_deref());
                    if selected.is_empty() {
                        return Err(Error::new("no child matched send down"));
                    }
                    for child_id in selected {
                        sends.push((child_id, value.clone(), Route::Down));
                    }
                }
            }
        }
        Ok(sends)
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

fn check_instance(program: &Program, node: &Instance) -> Result<(), Error> {
    let component = program
        .component(&node.component)
        .ok_or_else(|| Error::new(format!("unknown component {}", node.component)))?;
    let declared: HashMap<&str, &str> = component
        .ports
        .iter()
        .map(|port| (port.name.as_str(), port.component.as_str()))
        .collect();
    let mut filled = HashSet::new();
    for child in &node.children {
        if let Some(key) = child.key.as_deref() {
            if !filled.insert(key) {
                return Err(Error::new(format!("duplicate port fill {key}")));
            }
            match declared.get(key) {
                None => return Err(Error::new(format!("unknown port {key}"))),
                Some(kind) if *kind != child.component.as_str() => {
                    return Err(Error::new(format!(
                        "port {key} expects {kind} but scene filled {}",
                        child.component
                    )));
                }
                Some(_) => {}
            }
        }
        check_instance(program, child)?;
    }
    for port in &component.ports {
        if !filled.contains(port.name.as_str()) {
            return Err(Error::new(format!("missing port fill {}", port.name)));
        }
    }
    Ok(())
}
