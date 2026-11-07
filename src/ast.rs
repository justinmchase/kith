use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Program {
    pub kind: String,
    #[serde(default)]
    pub imports: Vec<Import>,
    #[serde(default)]
    pub exports: Vec<Export>,
    #[serde(default)]
    pub components: Vec<Component>,
    #[serde(default)]
    pub scenes: Vec<Scene>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Import {
    pub kind: String,
    #[serde(rename = "moduleUrl")]
    pub module_url: String,
    pub names: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Export {
    pub kind: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Component {
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub fields: Vec<Field>,
    #[serde(default)]
    pub ports: Vec<Port>,
    pub states: Vec<State>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Field {
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub pattern: Option<Json>,
    #[serde(default)]
    pub init: Option<Json>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Port {
    pub kind: String,
    pub component: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Scene {
    pub kind: String,
    pub name: String,
    pub root: Instance,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Instance {
    pub kind: String,
    pub component: String,
    #[serde(default)]
    pub key: Option<String>,
    #[serde(default)]
    pub children: Vec<Instance>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct State {
    pub kind: String,
    pub name: String,
    pub start: bool,
    #[serde(default)]
    pub handlers: Vec<Handler>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Handler {
    pub kind: String,
    pub pattern: Json,
    #[serde(default)]
    pub body: Vec<Statement>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum Statement {
    #[serde(rename = "assign")]
    Assign { field: String, expression: Json },
    #[serde(rename = "sendUp")]
    SendUp { message: Json },
    #[serde(rename = "sendTo")]
    SendTo { port: String, message: Json },
    #[serde(rename = "sendDown")]
    SendDown {
        component: Option<String>,
        message: Json,
    },
}

impl Program {
    pub fn component(&self, name: &str) -> Option<&Component> {
        self.components
            .iter()
            .find(|component| component.name == name)
    }

    pub fn scene(&self, name: &str) -> Option<&Scene> {
        self.scenes.iter().find(|scene| scene.name == name)
    }

    pub fn exported_names(&self) -> impl Iterator<Item = &str> {
        self.exports.iter().map(|export| export.name.as_str())
    }

    pub fn exported_scenes(&self) -> Vec<&Scene> {
        self.scenes
            .iter()
            .filter(|scene| self.exports.iter().any(|export| export.name == scene.name))
            .collect()
    }

    pub fn unique_exported_scene(&self) -> Result<&Scene, String> {
        let scenes = self.exported_scenes();
        match scenes.as_slice() {
            [scene] => Ok(scene),
            [] => Err("program must export exactly one scene".into()),
            _ => Err("program exports more than one scene".into()),
        }
    }
}

impl Component {
    pub fn start_state(&self) -> Option<&State> {
        let starts: Vec<_> = self.states.iter().filter(|state| state.start).collect();
        if starts.len() == 1 {
            Some(starts[0])
        } else {
            None
        }
    }

    pub fn port(&self, name: &str) -> Option<&Port> {
        self.ports.iter().find(|port| port.name == name)
    }
}

pub fn ping_program() -> Program {
    serde_json::from_value(serde_json::json!({
        "kind": "program",
        "imports": [],
        "exports": [
            {"kind": "export", "name": "Child"},
            {"kind": "export", "name": "Parent"},
            {"kind": "export", "name": "Main"}
        ],
        "components": [
            {
                "kind": "component",
                "name": "Child",
                "fields": [],
                "ports": [],
                "states": [{
                    "kind": "state",
                    "name": "Idle",
                    "start": true,
                    "handlers": [{
                        "kind": "handler",
                        "pattern": ping_pattern("Ping"),
                        "body": [{
                            "kind": "sendUp",
                            "message": ping_object("Pong")
                        }]
                    }]
                }]
            },
            {
                "kind": "component",
                "name": "Parent",
                "fields": [{
                    "kind": "field",
                    "name": "heard",
                    "pattern": {"kind": "type", "type": "number"},
                    "init": {"kind": "number", "value": 0}
                }],
                "ports": [{
                    "kind": "port",
                    "component": "Child",
                    "name": "child"
                }],
                "states": [{
                    "kind": "state",
                    "name": "Idle",
                    "start": true,
                    "handlers": [
                        {
                            "kind": "handler",
                            "pattern": ping_pattern("Ping"),
                            "body": [{
                                "kind": "sendTo",
                                "port": "child",
                                "message": ping_object("Ping")
                            }]
                        },
                        {
                            "kind": "handler",
                            "pattern": ping_pattern("Pong"),
                            "body": [{
                                "kind": "assign",
                                "field": "heard",
                                "expression": {"kind": "reference", "name": "n"}
                            }]
                        }
                    ]
                }]
            }
        ],
        "scenes": [{
            "kind": "scene",
            "name": "Main",
            "root": {
                "kind": "instance",
                "component": "Parent",
                "key": null,
                "children": [{
                    "kind": "instance",
                    "component": "Child",
                    "key": "child",
                    "children": []
                }]
            }
        }]
    }))
    .expect("ping program")
}

pub fn ping_pattern(kind: &str) -> serde_json::Value {
    serde_json::json!({
        "kind": "over",
        "keys": {
            "kind": {
                "kind": "equal",
                "value": {"kind": "value.literal", "value": kind}
            },
            "n": {
                "kind": "variable",
                "name": "n",
                "pattern": {"kind": "any"}
            }
        },
        "rest": null
    })
}

pub fn ping_object(kind: &str) -> serde_json::Value {
    serde_json::json!({
        "kind": "object",
        "keys": [
            {
                "kind": "objectKey",
                "name": "kind",
                "expression": {"kind": "string", "values": [kind]}
            },
            {
                "kind": "objectKey",
                "name": "n",
                "expression": {"kind": "reference", "name": "n"}
            }
        ]
    })
}
