use kith::dispatch::{ActorId, Dispatcher, Message};
use kith::doc;
use std::cell::RefCell;
use std::rc::Rc;

#[test]
// req:dispatch-001
fn serial_fifo_runs_one_ready_actor_per_turn() {
    let mut dispatcher = Dispatcher::new();
    let first = ActorId(1);
    let second = ActorId(2);
    dispatcher.register(first);
    dispatcher.register(second);
    dispatcher.send(
        first,
        Message {
            value: doc::string("a"),
        },
    );
    dispatcher.send(
        second,
        Message {
            value: doc::string("b"),
        },
    );
    dispatcher.send(
        first,
        Message {
            value: doc::string("c"),
        },
    );

    let order = Rc::new(RefCell::new(Vec::new()));
    let observed = order.clone();
    dispatcher
        .run_until_idle(|actor, message| {
            observed.borrow_mut().push((actor, message.value.clone()));
            Ok::<_, ()>(Vec::new())
        })
        .unwrap();

    assert_eq!(
        *order.borrow(),
        vec![
            (first, doc::string("a")),
            (second, doc::string("b")),
            (first, doc::string("c")),
        ]
    );
}

#[test]
// req:dispatch-002
fn self_send_is_taken_on_a_later_turn() {
    let mut dispatcher = Dispatcher::new();
    let actor = ActorId(1);
    dispatcher.register(actor);
    dispatcher.send(
        actor,
        Message {
            value: doc::string("start"),
        },
    );

    let turns = Rc::new(RefCell::new(Vec::new()));
    let observed = turns.clone();
    dispatcher
        .run_until_idle(|id, message| {
            observed.borrow_mut().push(message.value.clone());
            if doc::as_str(&message.value).as_deref() == Some("start") {
                Ok::<_, ()>(vec![(
                    id,
                    Message {
                        value: doc::string("self"),
                    },
                )])
            } else {
                Ok(Vec::new())
            }
        })
        .unwrap();

    assert_eq!(
        *turns.borrow(),
        vec![doc::string("start"), doc::string("self")]
    );
}
