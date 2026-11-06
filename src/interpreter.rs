use crate::ast::{Program, Statement};
use crate::dispatch::{ActorId, Dispatcher, Message};
use crate::error::Error;
use crate::host::{evaluate, match_message};
use serde_json::Value as Json;
use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    Inject,
    Down,
    Bubble,
}

struct Instance {
    kind: String,
    parent: Option<ActorId>,
    children: HashMap<String, ActorId>,
    fields: HashMap<String, Json>,
    state: String,
}

pub struct World {
    program: Program,
    instances: HashMap<ActorId, Instance>,
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
        if program.component(&program.root).is_none() {
            return Err(Error::new(format!("unknown root {}", program.root)));
        }
        for component in &program.components {
            if component.start_state().is_none() {
                return Err(Error::new(format!(
                    "component {} must declare exactly one start state",
                    component.name
                )));
            }
            for child in &component.children {
                if program.component(&child.component).is_none() {
                    return Err(Error::new(format!(
                        "unknown child kind {}",
                        child.component
                    )));
                }
            }
        }
        Ok(())
    }

    pub fn start(program: Program) -> Result<Self, Error> {
        Self::check(&program)?;
        let mut world = Self {
            program,
            instances: HashMap::new(),
            next_id: 1,
            dispatcher: Dispatcher::new(),
            routes: HashMap::new(),
            route_seq: 0,
        };
        let root = world.program.root.clone();
        world.spawn(root, None)?;
        Ok(world)
    }

    pub fn root_id(&self) -> ActorId {
        ActorId(1)
    }

    pub fn field(&self, id: ActorId, name: &str) -> Option<&Json> {
        self.instances.get(&id)?.fields.get(name)
    }

    pub fn child(&self, id: ActorId, slot: &str) -> Option<ActorId> {
        self.instances.get(&id)?.children.get(slot).copied()
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

    fn spawn(&mut self, kind: String, parent: Option<ActorId>) -> Result<ActorId, Error> {
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
            Instance {
                kind: kind.clone(),
                parent,
                children: HashMap::new(),
                fields,
                state: start.name.clone(),
            },
        );
        let child_decls = component.children.clone();
        for child in child_decls {
            let child_id = self.spawn(child.component.clone(), Some(id))?;
            self.instances
                .get_mut(&id)
                .expect("parent")
                .children
                .insert(child.name, child_id);
        }
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
            Route::Bubble => {
                if let Some(parent) = self.instances[&id].parent {
                    Ok(vec![(parent, message.clone(), Route::Bubble)])
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
                Statement::Bubble { message } => {
                    let value = evaluate(message, bindings)?;
                    let parent = self.instances[&id]
                        .parent
                        .ok_or_else(|| Error::new("bubble from root"))?;
                    sends.push((parent, value, Route::Bubble));
                }
                Statement::Down { child, message } => {
                    let value = evaluate(message, bindings)?;
                    let child_id = *self.instances[&id]
                        .children
                        .get(child)
                        .ok_or_else(|| Error::new(format!("unknown child {child}")))?;
                    sends.push((child_id, value, Route::Down));
                }
                Statement::DownAll { message } => {
                    let value = evaluate(message, bindings)?;
                    let children: Vec<_> = self.instances[&id].children.values().copied().collect();
                    for child_id in children {
                        sends.push((child_id, value.clone(), Route::Down));
                    }
                }
            }
        }
        Ok(sends)
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
