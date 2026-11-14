use serde_json::{json, Value as Json};

#[derive(Clone, Debug, PartialEq)]
pub enum Pipe {
    /// Open or closed, with at most one queued value, and no pull waiting.
    Idle { open: bool, queued: Option<Json> },
    /// An open empty pipe with one `"Pull"` that has not been answered.
    Waiting,
}

impl Pipe {
    pub fn new() -> Self {
        Self::Idle {
            open: true,
            queued: None,
        }
    }
}

pub fn step(pipe: Pipe, message: &Json) -> (Pipe, Vec<Json>) {
    match pipe {
        Pipe::Waiting => match command(message) {
            Command::Pull => (Pipe::Waiting, vec![error("busy")]),
            Command::Push(value) => (
                Pipe::Idle {
                    open: true,
                    queued: None,
                },
                vec![chunk(value), json!("Pushed")],
            ),
            Command::Close => (
                Pipe::Idle {
                    open: false,
                    queued: None,
                },
                vec![json!("End"), json!("Closed")],
            ),
            Command::Unknown => (Pipe::Waiting, vec![error("unknown")]),
        },
        Pipe::Idle { open, queued } => match command(message) {
            Command::Pull => match queued {
                Some(value) => (Pipe::Idle { open, queued: None }, vec![chunk(value)]),
                None if open => (Pipe::Waiting, vec![]),
                None => (Pipe::Idle { open, queued }, vec![json!("End")]),
            },
            Command::Push(value) => {
                if !open {
                    return (Pipe::Idle { open, queued }, vec![error("closed")]);
                }
                if queued.is_some() {
                    return (Pipe::Idle { open, queued }, vec![error("full")]);
                }
                (
                    Pipe::Idle {
                        open: true,
                        queued: Some(value),
                    },
                    vec![json!("Pushed")],
                )
            }
            Command::Close => {
                if !open {
                    return (Pipe::Idle { open, queued }, vec![error("closed")]);
                }
                (
                    Pipe::Idle {
                        open: false,
                        queued,
                    },
                    vec![json!("Closed")],
                )
            }
            Command::Unknown => (Pipe::Idle { open, queued }, vec![error("unknown")]),
        },
    }
}

enum Command {
    Pull,
    Push(Json),
    Close,
    Unknown,
}

fn command(message: &Json) -> Command {
    match message {
        Json::String(text) if text == "Pull" => Command::Pull,
        Json::String(text) if text == "Close" => Command::Close,
        Json::Array(items) if items.len() == 2 && items[0] == json!("Push") => {
            Command::Push(items[1].clone())
        }
        _ => Command::Unknown,
    }
}

fn chunk(value: Json) -> Json {
    json!(["Chunk", value])
}

fn error(reason: &str) -> Json {
    json!(["Error", reason])
}

pub fn to_json(pipe: &Pipe) -> Json {
    match pipe {
        Pipe::Idle { open, queued } => json!({
            "kind": "pipe",
            "open": open,
            "queued": queued,
            "waiting": false,
        }),
        Pipe::Waiting => json!({
            "kind": "pipe",
            "open": true,
            "queued": null,
            "waiting": true,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // req:builtins-003
    fn waiting_pull_is_answered_by_push_and_a_second_pull_is_busy() {
        let (waiting, none) = step(Pipe::new(), &json!("Pull"));
        assert!(none.is_empty());
        assert_eq!(waiting, Pipe::Waiting);
        let (still, busy) = step(waiting.clone(), &json!("Pull"));
        assert_eq!(still, Pipe::Waiting);
        assert_eq!(busy, vec![json!(["Error", "busy"])]);
        let (idle, replies) = step(waiting, &json!(["Push", 7]));
        assert_eq!(
            idle,
            Pipe::Idle {
                open: true,
                queued: None
            }
        );
        assert_eq!(replies, vec![json!(["Chunk", 7]), json!("Pushed")]);
        let (closed, end) = step(Pipe::Waiting, &json!("Close"));
        assert_eq!(end, vec![json!("End"), json!("Closed")]);
        assert_eq!(
            closed,
            Pipe::Idle {
                open: false,
                queued: None
            }
        );
        let queued = Pipe::Idle {
            open: true,
            queued: Some(json!("x")),
        };
        let (held, closed_reply) = step(queued, &json!("Close"));
        assert_eq!(closed_reply, vec![json!("Closed")]);
        let (empty, chunk) = step(held, &json!("Pull"));
        assert_eq!(chunk, vec![json!(["Chunk", "x"])]);
        let (_, done) = step(empty, &json!("Pull"));
        assert_eq!(done, vec![json!("End")]);
    }
}

pub fn from_json(value: &Json) -> Option<Pipe> {
    if value.get("kind")?.as_str()? != "pipe" {
        return None;
    }
    if value.get("waiting")?.as_bool()? {
        return Some(Pipe::Waiting);
    }
    Some(Pipe::Idle {
        open: value.get("open")?.as_bool()?,
        queued: value.get("queued").filter(|item| !item.is_null()).cloned(),
    })
}
