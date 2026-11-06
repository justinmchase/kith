use serde::{Deserialize, Serialize};
use serde_json::Value as Json;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Program {
    pub kind: String,
    pub components: Vec<Component>,
    pub root: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Component {
    pub kind: String,
    pub name: String,
    #[serde(default)]
    pub fields: Vec<Field>,
    #[serde(default)]
    pub children: Vec<Child>,
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
pub struct Child {
    pub kind: String,
    pub component: String,
    pub name: String,
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
    #[serde(rename = "bubble")]
    Bubble { message: Json },
    #[serde(rename = "down")]
    Down { child: String, message: Json },
    #[serde(rename = "downAll")]
    DownAll { message: Json },
}

impl Program {
    pub fn component(&self, name: &str) -> Option<&Component> {
        self.components
            .iter()
            .find(|component| component.name == name)
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
}

pub fn ping_program() -> Program {
    serde_json::from_value(serde_json::json!({
        "kind": "program",
        "root": "Parent",
        "components": [
            {
                "kind": "component",
                "name": "Child",
                "fields": [],
                "children": [],
                "states": [{
                    "kind": "state",
                    "name": "Idle",
                    "start": true,
                    "handlers": [{
                        "kind": "handler",
                        "pattern": ping_pattern("Ping"),
                        "body": [{
                            "kind": "bubble",
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
                "children": [{
                    "kind": "child",
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
                                "kind": "down",
                                "child": "child",
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
        ]
    }))
    .expect("ping program")
}

fn ping_pattern(kind: &str) -> serde_json::Value {
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

fn ping_object(kind: &str) -> serde_json::Value {
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
