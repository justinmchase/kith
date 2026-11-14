---
id: components-007
title: print records a diagnostic value without completing the instance
spec_ref: .agents/specifications/components.spec.md#sending
---

# Print

## Requirement

Preconditions:

- A component transition includes `print Expression`.

Expected behavior:

- Evaluating the transition MUST record the value on the world. The instance
  MUST NOT complete because of the print.
- Order's Carrier pickup MUST print `{ kind: "PickupRequested", ... }` twice
  before any delivery confirmation.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-007`).
