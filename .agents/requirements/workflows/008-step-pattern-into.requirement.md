---
id: workflows-008
title: A step may destructure the working value and initialize the accumulator
spec_ref: .agents/specifications/workflows.spec.md#declaration
---

# Step pattern and into init

## Requirement

Preconditions:

- `Fulfill` in `examples/order.kith` is
  `{ items: any, orderId: any } -> fanout items with Carrier into Summarize(orderId) then succeed`.

Expected behavior:

- The step pattern MUST be a Uffda Over that binds `items` and `orderId` from
  a Paid message and MUST ignore extra keys such as `kind`.
- `fanout items` MUST map that list. `Summarize(orderId)` MUST set
  Summarize's `orderId` field.
- After two deliveries the run MUST succeed with
  `{ kind: "Summarize", orderId: "o-1", total: 2 }`.
- `then succeed` is Next; a fanout step MAY `then Name` instead.

Postconditions:

- Tests: `tests/grammar.rs` and `tests/interpreter.rs` (`req:workflows-008`).
