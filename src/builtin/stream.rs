use crate::doc::{self, Doc};

#[derive(Clone, Debug, PartialEq)]
pub enum Pipe {
    /// Open or closed, with at most one queued value, and no pull waiting.
    Idle { open: bool, queued: Option<Doc> },
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

pub fn step(pipe: Pipe, message: &Doc) -> (Pipe, Vec<Doc>) {
    match pipe {
        Pipe::Waiting => match command(message) {
            Command::Pull => (Pipe::Waiting, vec![error("busy")]),
            Command::Push(value) => (
                Pipe::Idle {
                    open: true,
                    queued: None,
                },
                vec![chunk(value), doc::string("Pushed")],
            ),
            Command::Close => (
                Pipe::Idle {
                    open: false,
                    queued: None,
                },
                vec![doc::string("End"), doc::string("Closed")],
            ),
            Command::Unknown => (Pipe::Waiting, vec![error("unknown")]),
        },
        Pipe::Idle { open, queued } => match command(message) {
            Command::Pull => match queued {
                Some(value) => (Pipe::Idle { open, queued: None }, vec![chunk(value)]),
                None if open => (Pipe::Waiting, vec![]),
                None => (Pipe::Idle { open, queued }, vec![doc::string("End")]),
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
                    vec![doc::string("Pushed")],
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
                    vec![doc::string("Closed")],
                )
            }
            Command::Unknown => (Pipe::Idle { open, queued }, vec![error("unknown")]),
        },
    }
}

enum Command {
    Pull,
    Push(Doc),
    Close,
    Unknown,
}

fn command(message: &Doc) -> Command {
    if doc::as_str(message).as_deref() == Some("Pull") {
        return Command::Pull;
    }
    if doc::as_str(message).as_deref() == Some("Close") {
        return Command::Close;
    }
    if let Some(items) = doc::items(message) {
        if items.len() == 2 && doc::as_str(&items[0]).as_deref() == Some("Push") {
            return Command::Push(items[1].clone());
        }
    }
    Command::Unknown
}

fn chunk(value: Doc) -> Doc {
    doc::array([doc::string("Chunk"), value])
}

fn error(reason: &str) -> Doc {
    doc::array([doc::string("Error"), doc::string(reason)])
}

pub fn to_document(pipe: &Pipe) -> Doc {
    match pipe {
        Pipe::Idle { open, queued } => doc::object([
            ("kind", doc::string("pipe")),
            ("open", doc::boolean(*open)),
            ("queued", queued.clone().unwrap_or_else(doc::null)),
            ("waiting", doc::boolean(false)),
        ]),
        Pipe::Waiting => doc::object([
            ("kind", doc::string("pipe")),
            ("open", doc::boolean(true)),
            ("queued", doc::null()),
            ("waiting", doc::boolean(true)),
        ]),
    }
}

pub fn from_document(value: &Doc) -> Option<Pipe> {
    if doc::as_str(&doc::get(value, "kind")?).as_deref() != Some("pipe") {
        return None;
    }
    if doc::as_bool(&doc::get(value, "waiting")?)? {
        return Some(Pipe::Waiting);
    }
    let queued = doc::get(value, "queued").filter(|item| !doc::is_null(item));
    Some(Pipe::Idle {
        open: doc::as_bool(&doc::get(value, "open")?)?,
        queued,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    // req:builtins-003
    fn waiting_pull_is_answered_by_push_and_a_second_pull_is_busy() {
        let (waiting, none) = step(Pipe::new(), &doc::string("Pull"));
        assert!(none.is_empty());
        assert_eq!(waiting, Pipe::Waiting);
        let (still, busy) = step(waiting.clone(), &doc::string("Pull"));
        assert_eq!(still, Pipe::Waiting);
        assert_eq!(
            busy,
            vec![doc::array([doc::string("Error"), doc::string("busy")])]
        );
        let (idle, replies) = step(
            waiting,
            &doc::array([doc::string("Push"), doc::number(7.0)]),
        );
        assert_eq!(
            idle,
            Pipe::Idle {
                open: true,
                queued: None
            }
        );
        assert_eq!(
            replies,
            vec![
                doc::array([doc::string("Chunk"), doc::number(7.0)]),
                doc::string("Pushed"),
            ]
        );
        let (closed, end) = step(Pipe::Waiting, &doc::string("Close"));
        assert_eq!(end, vec![doc::string("End"), doc::string("Closed")]);
        assert_eq!(
            closed,
            Pipe::Idle {
                open: false,
                queued: None
            }
        );
        let queued = Pipe::Idle {
            open: true,
            queued: Some(doc::string("x")),
        };
        let (held, closed_reply) = step(queued, &doc::string("Close"));
        assert_eq!(closed_reply, vec![doc::string("Closed")]);
        let (empty, chunk) = step(held, &doc::string("Pull"));
        assert_eq!(
            chunk,
            vec![doc::array([doc::string("Chunk"), doc::string("x")])]
        );
        let (_, done) = step(empty, &doc::string("Pull"));
        assert_eq!(done, vec![doc::string("End")]);
    }
}
