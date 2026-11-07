use serde_json::Value as Json;
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ActorId(pub u64);

#[derive(Clone, Debug)]
pub struct Message {
    pub value: Json,
}

pub struct Dispatcher {
    mailboxes: HashMap<ActorId, VecDeque<Message>>,
    ready: VecDeque<ActorId>,
}

#[derive(Debug)]
pub struct Turn {
    pub actor: ActorId,
    pub message: Message,
}

impl Dispatcher {
    pub fn new() -> Self {
        Self {
            mailboxes: HashMap::new(),
            ready: VecDeque::new(),
        }
    }

    pub fn register(&mut self, actor: ActorId) {
        self.mailboxes.entry(actor).or_default();
    }

    pub fn send(&mut self, to: ActorId, message: Message) {
        let mailbox = self.mailboxes.entry(to).or_default();
        let was_empty = mailbox.is_empty();
        mailbox.push_back(message);
        if was_empty {
            self.ready.push_back(to);
        }
    }

    pub fn turn(&mut self) -> Option<Turn> {
        let actor = self.ready.pop_front()?;
        let mailbox = self.mailboxes.get_mut(&actor)?;
        let message = mailbox.pop_front()?;
        if !mailbox.is_empty() {
            self.ready.push_back(actor);
        }
        Some(Turn { actor, message })
    }

    pub fn run_until_idle<F, E>(&mut self, mut handle: F) -> Result<(), E>
    where
        F: FnMut(ActorId, &Message) -> Result<Vec<(ActorId, Message)>, E>,
    {
        while let Some(turn) = self.turn() {
            let sends = handle(turn.actor, &turn.message)?;
            for (to, message) in sends {
                self.send(to, message);
            }
        }
        Ok(())
    }
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}
