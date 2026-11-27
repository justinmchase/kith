---
id: components-022
title: An unreachable instance is destroyed between turns with its descendants
spec_ref: .agents/specifications/components.spec.md#identity-and-lifetime; .agents/specifications/dispatch.spec.md#routing; .agents/specifications/dispatch.spec.md#transition-actors
---

# Unreachable instances are destroyed

## Requirement

Preconditions:

- An instance is the root, or its parent is reachable and still holds it
  in the child collection, or a reachable instance stores it in a field.
- The parent holds each scene-constructed child, port or unnamed, for the
  parent's whole life. It holds a dynamic child until a transition of that
  parent releases it, or the parent is destroyed. A field that stores an
  instance holds that instance. The store is not a child and does not fill
  a port.
- A release is `{ kind: "release", filter: Pattern }`. The pattern matches
  the record the parent holds: `port` is the port name, or null when the
  child is unnamed, and `key` is null. The source spelling is open.

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
- A release MUST NOT drop a scene-constructed child. A release that would
  do so MUST fail with a diagnostic and MUST NOT drop a child. Zero matches
  MUST fail that release. The pattern MUST NOT see the child's fields,
  machine state, or mailbox, and MUST NOT compare the child to a named
  component. Collection is unreachability only. Failure, restart, and
  poison messages stay the planned supervision chapter.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-022`). A
  release drops a dynamic child and its descendants between turns, cancels
  an in-flight send to that child, and leaves a scene-constructed child in
  place. `complete` does not destroy a child. A `send up` the parent already
  handled stays handled. A message the destroyed child had not taken is not
  delivered.
