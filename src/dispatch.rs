use crate::doc::{self, Doc};
use std::collections::{HashMap, VecDeque};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub struct ActorId(pub u64);

#[derive(Clone, Debug)]
pub struct Message {
    pub value: Doc,
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
    pub fn snapshot(&self) -> Doc {
        let mailboxes = doc::object(self.mailboxes.iter().map(|(ActorId(id), queue)| {
            (
                id.to_string(),
                doc::array(queue.iter().map(|message| message.value.clone())),
            )
        }));
        let ready = doc::array(self.ready.iter().map(|ActorId(id)| doc::number(*id as f64)));
        doc::object([("mailboxes", mailboxes), ("ready", ready)])
    }

    pub fn restore(value: &Doc) -> Result<Self, String> {
        let mut dispatcher = Self::new();
        if let Some(mailboxes) =
            doc::entries(&doc::get(value, "mailboxes").unwrap_or_else(doc::null))
        {
            for (key, queue) in mailboxes {
                let id = ActorId(key.parse().map_err(|_| "bad actor id")?);
                dispatcher.register(id);
                let mailbox = dispatcher.mailboxes.entry(id).or_default();
                mailbox.clear();
                if let Some(values) = doc::items(&queue) {
                    for value in values {
                        mailbox.push_back(Message { value });
                    }
                }
            }
        }
        dispatcher.ready.clear();
        if let Some(ready) = doc::items(&doc::get(value, "ready").unwrap_or_else(doc::null)) {
            for id in ready {
                let id = doc::as_u64(&id).ok_or("bad ready id")?;
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
