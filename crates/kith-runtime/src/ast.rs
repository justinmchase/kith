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
    /// Pure functions bound from a built-in import.
    #[serde(default)]
    pub functions: Vec<String>,
    /// Pattern rules bound from a built-in import.
    #[serde(default)]
    pub rules: Vec<String>,
    /// Component kinds whose instances are host drivers.
    #[serde(default)]
    pub host_kinds: Vec<String>,
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
    /// Omitted means the field exists in every state, writable, with `pre` and
    /// `post` equal to `pattern`. The first clause opts the field into only
    /// the states those clauses name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rules: Option<Vec<FieldRule>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FieldRule {
    pub states: Vec<String>,
    #[serde(default)]
    pub write: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pre: Option<Json>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub post: Option<Json>,
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

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct State {
    pub kind: String,
    pub name: String,
    pub start: bool,
    /// `false` for unnamed `start { }`: the implicit name `Start` is not a `goto` target.
    #[serde(default = "default_true")]
    pub named: bool,
    #[serde(default)]
    pub handlers: Vec<Handler>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Handler {
    pub kind: String,
    pub pattern: Json,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub item: Option<Json>,
    #[serde(default)]
    pub route: Vec<String>,
    #[serde(default)]
    pub body: Vec<Statement>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "kind")]
pub enum Statement {
    #[serde(rename = "assign")]
    Assign { field: String, expression: Json },
    #[serde(rename = "sendUp")]
    SendUp {
        message: Json,
        #[serde(default)]
        after: Option<Json>,
    },
    #[serde(rename = "sendTo")]
    SendTo {
        port: String,
        message: Json,
        #[serde(default)]
        after: Option<Json>,
    },
    #[serde(rename = "sendDown")]
    SendDown {
        /// Always null. `send down` does not name a component.
        component: Option<String>,
        message: Json,
        #[serde(default)]
        after: Option<Json>,
    },
    #[serde(rename = "construct")]
    Construct {
        component: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        key: Option<Json>,
        #[serde(default)]
        children: Vec<Instance>,
    },
    #[serde(rename = "release")]
    Release { filter: Json },
    #[serde(rename = "send")]
    Send {
        message: Json,
        #[serde(default)]
        after: Option<Json>,
    },
    #[serde(rename = "complete")]
    Complete { message: Json },
    #[serde(rename = "expr")]
    Expr { expression: Json },
    #[serde(rename = "goto")]
    Goto { state: String },
}

pub struct Kind<'a> {
    pub name: &'a str,
    pub fields: &'a [Field],
    pub ports: &'a [Port],
}

impl Program {
    pub fn component(&self, name: &str) -> Option<&Component> {
        self.components
            .iter()
            .find(|component| component.name == name)
    }

    pub fn kind(&self, name: &str) -> Option<Kind<'_>> {
        let component = self.component(name)?;
        Some(Kind {
            name: component.name.as_str(),
            fields: &component.fields,
            ports: &component.ports,
        })
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
            {"kind": "export", "name": "Ping"},
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
                "name": "Ping",
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
                "component": "Ping",
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
        "kind": "then",
        "patterns": [
            {"kind": "symbol", "name": kind},
            {
                "kind": "variable",
                "name": "n",
                "pattern": {"kind": "type", "type": "number"}
            }
        ]
    })
}

pub fn ping_object(kind: &str) -> serde_json::Value {
    serde_json::json!({
        "kind": "sequence",
        "items": [
            {"kind": "symbol", "name": kind},
            {"kind": "reference", "name": "n"}
        ]
    })
}
