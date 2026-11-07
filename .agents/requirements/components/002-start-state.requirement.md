---
id: components-002
title: Each component has exactly one start state
spec_ref: .agents/specifications/components.spec.md#behavior
---

# One start state

## Requirement

Preconditions:

- A component declaration lists states.

Expected behavior:

- A component with exactly one `start` state MUST begin there.
- A component with zero or more than one start state MUST be rejected by
  `check` and MUST NOT run.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-002`).
