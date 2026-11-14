use crate::codec::json_u64;
use serde_json::{json, Value as Json};
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

impl Dispatcher {
    pub fn snapshot(&self) -> Json {
        let mut mailboxes = serde_json::Map::new();
        for (ActorId(id), queue) in &self.mailboxes {
            let values: Vec<Json> = queue.iter().map(|message| message.value.clone()).collect();
            mailboxes.insert(id.to_string(), Json::Array(values));
        }
        let ready: Vec<Json> = self
            .ready
            .iter()
            .map(|ActorId(id)| Json::from(*id))
            .collect();
        json!({ "mailboxes": mailboxes, "ready": ready })
    }

    pub fn restore(value: &Json) -> Result<Self, String> {
        let mut dispatcher = Self::new();
        if let Some(mailboxes) = value.get("mailboxes").and_then(Json::as_object) {
            for (key, queue) in mailboxes {
                let id = ActorId(key.parse().map_err(|_| "bad actor id")?);
                dispatcher.register(id);
                let mailbox = dispatcher.mailboxes.entry(id).or_default();
                mailbox.clear();
                if let Some(values) = queue.as_array() {
                    for value in values {
                        mailbox.push_back(Message {
                            value: value.clone(),
                        });
                    }
                }
            }
        }
        dispatcher.ready.clear();
        if let Some(ready) = value.get("ready").and_then(Json::as_array) {
            for id in ready {
                let id = json_u64(id).ok_or("bad ready id")?;
                dispatcher.ready.push_back(ActorId(id));
            }
        }
        Ok(dispatcher)
    }
}

impl Default for Dispatcher {
    fn default() -> Self {
        Self::new()
    }
}
