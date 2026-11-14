---
id: components-007
title: writeLine to stdout records a diagnostic value without completing the instance
spec_ref: .agents/specifications/components.spec.md#handler-expressions
---

# writeLine diagnostic

## Requirement

Preconditions:

- A component transition includes `(writeLine stdout Expression)` or
  `(print Expression)`.

Expected behavior:

- Evaluating the transition MUST record the value on the world. The instance
  MUST NOT complete because of the write.
- `(print V)` MUST be the same as `(writeLine stdout V)`.
- Order's Carrier pickup MUST write `{ kind: "PickupRequested", ... }` twice
  before any delivery confirmation.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-007`).
