---
id: workflows-003
title: fanout 0, 1, or N workers joins into an array output
spec_ref: .agents/specifications/workflows.spec.md#definitions
---

# Map fan-out and fan-in

## Requirement

Preconditions:

- A workflow `start fanout items -> Worker then succeed` and a port-less
  `Worker` component.

Expected behavior:

- Zero items MUST succeed with `[]`.
- One or more items MUST each receive `{ kind: "Item", item, index }` and
  appear in the succeeded array output.
- Fan-out of a kind that needs ports without a prototype MUST fail check.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:workflows-003`).
