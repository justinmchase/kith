use crate::ast::{Instance, KindSort, Program, Statement, Workflow};
use crate::dispatch::{ActorId, Dispatcher, Message};
use crate::error::Error;
use crate::host::{evaluate, match_message};
use serde_json::{json, Value as Json};
use std::collections::{HashMap, HashSet};

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
    results: Vec<Json>,
    total: u64,
    completed: u64,
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
        if program.workflow(&scene.root.component).is_none() {
            return Err(Error::new("scene root must be a workflow"));
        }
        for component in &program.components {
            if component.start_state().is_none() {
                return Err(Error::new(format!(
                    "component {} must declare exactly one start state",
                    component.name
                )));
            }
            check_ports(program, &component.ports)?;
            for state in &component.states {
                for handler in &state.handlers {
                    check_statements(program, None, &handler.body)?;
                }
            }
        }
        for workflow in &program.workflows {
            if workflow.start_step().is_none() {
                return Err(Error::new(format!(
                    "workflow {} must declare exactly one start step",
                    workflow.name
                )));
            }
            check_ports(program, &workflow.ports)?;
            for step in &workflow.steps {
                check_statements(program, Some(workflow), &step.enter)?;
                for handler in &step.handlers {
                    check_statements(program, Some(workflow), &handler.body)?;
                }
                if let Some(join) = &step.join {
                    check_statements(program, Some(workflow), &join.body)?;
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
        let id = self.spawn_kind(&node.component, parent)?;
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
        self.run_enter(id)?;
        Ok(id)
    }

    fn spawn_kind(&mut self, kind: &str, parent: Option<ActorId>) -> Result<ActorId, Error> {
        let (sort, fields_decl) = {
            let declared = self
                .program
                .kind(kind)
                .ok_or_else(|| Error::new(format!("unknown kind {kind}")))?;
            (declared.sort, declared.fields.to_vec())
        };
        let start = match sort {
            KindSort::Component => self
                .program
                .component(kind)
                .and_then(|component| component.start_state())
                .map(|state| state.name.clone()),
            KindSort::Workflow => self
                .program
                .workflow(kind)
                .and_then(|workflow| workflow.start_step())
                .map(|step| step.name.clone()),
        }
        .ok_or_else(|| Error::new(format!("kind {kind} has no start")))?;
        let id = ActorId(self.next_id);
        self.next_id += 1;
        self.dispatcher.register(id);
        let mut fields = HashMap::new();
        for field in fields_decl {
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
            },
        );
        Ok(id)
    }

    fn run_enter(&mut self, id: ActorId) -> Result<(), Error> {
        let Some(enter) = self
            .current_workflow_step(id)
            .map(|step| step.enter.clone())
        else {
            return Ok(());
        };
        if enter.is_empty() {
            return Ok(());
        }
        let bindings = self.instances[&id].fields.clone();
        let sends = self.run_statements(id, &enter, &bindings)?;
        for (to, message, route) in sends {
            self.deliver(to, message, route)?;
        }
        Ok(())
    }

    fn current_workflow_step(&self, id: ActorId) -> Option<&crate::ast::Step> {
        let instance = self.instances.get(&id)?;
        self.program.workflow(&instance.kind)?.step(&instance.state)
    }

    fn handle(
        &mut self,
        id: ActorId,
        message: &Json,
        route: Route,
    ) -> Result<Vec<(ActorId, Json, Route)>, Error> {
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
        if let Some(workflow) = self.program.workflow(&kind_name).cloned() {
            return self.handle_workflow(id, &workflow, &state_name, &fields, message, route);
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
            if let Some(mut bindings) = match_message(&handler.pattern, message)? {
                for (name, value) in &fields {
                    bindings.entry(name.clone()).or_insert(value.clone());
                }
                return self.run_statements(id, &handler.body, &bindings);
            }
        }
        unmatched(route, &kind_name, self.instances[&id].parent, message)
    }

    fn handle_workflow(
        &mut self,
        id: ActorId,
        workflow: &Workflow,
        state_name: &str,
        fields: &HashMap<String, Json>,
        message: &Json,
        route: Route,
    ) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        let step = workflow
            .step(state_name)
            .ok_or_else(|| Error::new("unknown step"))?;
        if route == Route::Join {
            let join = step
                .join
                .as_ref()
                .ok_or_else(|| Error::new(format!("no join handler on {}", workflow.name)))?;
            let mut bindings = fields.clone();
            if let Some(results) = message.get("results") {
                bindings.insert("results".into(), results.clone());
            }
            return self.run_statements(id, &join.body, &bindings);
        }
        for handler in &step.handlers {
            if let Some(mut bindings) = match_message(&handler.pattern, message)? {
                for (name, value) in fields {
                    bindings.entry(name.clone()).or_insert(value.clone());
                }
                return self.run_statements(id, &handler.body, &bindings);
            }
        }
        unmatched(route, &workflow.name, self.instances[&id].parent, message)
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
                Statement::Complete { message } => {
                    let value = evaluate(message, &bindings)?;
                    {
                        let instance = self.instances.get_mut(&id).expect("actor");
                        instance.completed = true;
                        instance.completion = Some(value.clone());
                    }
                    if let Some(parent) = self.instances[&id].parent {
                        sends.push((parent, value.clone(), Route::Up));
                        if let Some(joined) = self.note_complete(parent, id, value) {
                            sends.push(joined);
                        }
                    }
                }
                Statement::Fanout {
                    collection,
                    component,
                } => {
                    sends.extend(self.fanout(id, collection, component, &bindings)?);
                }
                Statement::Goto { step } => {
                    sends.extend(self.goto_step(id, step, &bindings)?);
                }
            }
        }
        Ok(sends)
    }

    fn fanout(
        &mut self,
        id: ActorId,
        collection: &Json,
        component: &str,
        bindings: &HashMap<String, Json>,
    ) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        let value = evaluate(collection, bindings)?;
        let items = value
            .as_array()
            .ok_or_else(|| Error::new("fanout expects a list"))?;
        let mut remaining = HashSet::new();
        let mut sends = Vec::new();
        for (index, item) in items.iter().enumerate() {
            let child_id = self.spawn_kind(component, Some(id))?;
            self.run_enter(child_id)?;
            remaining.insert(child_id);
            self.instances
                .get_mut(&id)
                .expect("actor")
                .children
                .push(ChildRef {
                    id: child_id,
                    key: None,
                    component: component.to_string(),
                });
            sends.push((
                child_id,
                json!({
                    "kind": "Item",
                    "item": item,
                    "index": index as f64,
                }),
                Route::Down,
            ));
        }
        let total = remaining.len() as u64;
        self.instances.get_mut(&id).expect("actor").join = Some(JoinSet {
            remaining: remaining.clone(),
            results: Vec::new(),
            total,
            completed: 0,
        });
        if remaining.is_empty() {
            sends.push((id, json!({"kind": "Joined", "results": []}), Route::Join));
        }
        Ok(sends)
    }

    fn goto_step(
        &mut self,
        id: ActorId,
        step_name: &str,
        bindings: &HashMap<String, Json>,
    ) -> Result<Vec<(ActorId, Json, Route)>, Error> {
        let kind = self.instances[&id].kind.clone();
        let workflow = self
            .program
            .workflow(&kind)
            .ok_or_else(|| Error::new("goto is only valid on a workflow"))?;
        let step = workflow
            .step(step_name)
            .ok_or_else(|| Error::new(format!("unknown step {step_name}")))?
            .clone();
        self.instances.get_mut(&id).expect("actor").state = step.name.clone();
        self.run_statements(id, &step.enter, bindings)
    }

    fn note_complete(
        &mut self,
        parent: ActorId,
        child: ActorId,
        payload: Json,
    ) -> Option<(ActorId, Json, Route)> {
        let join = self.instances.get_mut(&parent)?.join.as_mut()?;
        if !join.remaining.remove(&child) {
            return None;
        }
        join.results.push(payload);
        join.completed += 1;
        if !join.remaining.is_empty() {
            return None;
        }
        let results = Json::Array(std::mem::take(&mut join.results));
        self.instances.get_mut(&parent)?.join = None;
        Some((
            parent,
            json!({"kind": "Joined", "results": results}),
            Route::Join,
        ))
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

fn check_statements(
    program: &Program,
    workflow: Option<&Workflow>,
    statements: &[Statement],
) -> Result<(), Error> {
    for statement in statements {
        match statement {
            Statement::Fanout { component, .. } => {
                if workflow.is_none() {
                    return Err(Error::new("component must not fanout"));
                }
                let kind = program
                    .kind(component)
                    .ok_or_else(|| Error::new(format!("unknown fanout kind {component}")))?;
                if !kind.ports.is_empty() {
                    return Err(Error::new(format!(
                        "fanout kind {component} must not declare ports"
                    )));
                }
            }
            Statement::Goto { step } => {
                let workflow = workflow.ok_or_else(|| Error::new("component must not goto"))?;
                if workflow.step(step).is_none() {
                    return Err(Error::new(format!("unknown step {step}")));
                }
            }
            Statement::Complete { .. }
            | Statement::Assign { .. }
            | Statement::SendUp { .. }
            | Statement::SendTo { .. }
            | Statement::SendDown { .. } => {}
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
    Ok(())
}
