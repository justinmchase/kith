---
id: components-022
title: An unreachable instance is destroyed between turns with its descendants
spec_ref: .agents/specifications/components.spec.md#identity-and-lifetime; .agents/specifications/dispatch.spec.md#routing; .agents/specifications/dispatch.spec.md#transition-actors
---

# Unreachable instances are destroyed

## Requirement

Preconditions:

- An instance is the root, or its parent is reachable and still holds it.
- The parent holds each scene-constructed child, port or unnamed, for the
  parent's whole life. It holds a dynamic child until a transition of that
  parent releases it, or the parent is destroyed.

Expected behavior:

- An unreachable instance MUST be destroyed, and its descendants with it.
  Reachability is what destroys an instance. Destruction runs between turns.
- Destruction MUST cancel that instance's in-flight transition actors,
  drop messages that instance has not taken, and drop later routes to it.
  Cancelled work MUST NOT enqueue a successful outcome. It MUST run no
  transition, send nothing, and offer no init. It is not a handler the
  instance matches.
- `complete` MUST NOT release a child and MUST NOT destroy it.
- A `send up` already delivered into the parent's mailbox MUST stay.
  Destruction MUST NOT pull a message back out of a mailbox that already
  has it.
- A release MUST NOT drop a scene-constructed child. Collection is
  unreachability only. Failure, restart, and poison messages stay the
  planned supervision chapter.

Postconditions:

- Tests: `tests/lifetime.rs` (`req:components-022`). A release drops a
  dynamic child and its descendants between turns, cancels an in-flight
  send to that child, and leaves a scene-constructed child in place.
  `complete` does not destroy a child. A `send up` the parent already
  handled stays handled.
