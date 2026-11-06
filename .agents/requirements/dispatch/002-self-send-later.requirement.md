---
id: dispatch-002
title: A send to self is taken on a later turn
spec_ref: .agents/specifications/dispatch.spec.md#component-actors
---

# Self-send later turn

## Requirement

Preconditions:

- An actor, during a turn, sends a message to itself.

Expected behavior:

- The self-send MUST NOT be dequeued during the same turn.
- After the turn completes, the actor MUST be ready and MUST take the message
  on a later turn.

Postconditions:

- Tests: `tests/dispatch.rs` (`req:dispatch-002`).
