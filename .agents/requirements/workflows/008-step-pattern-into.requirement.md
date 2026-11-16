---
id: workflows-008
title: A step may destructure the working value and initialize the accumulator
spec_ref: .agents/specifications/workflows.spec.md#declaration
---

# Step pattern and into init

## Requirement

Preconditions:

- `fulfill` in `examples/order.kith` is
  `{ items: any, orderId: any } fanout items with Carrier into Summarize(orderId) -> succeed`.
  The `paid` arm that reaches it is
  `({ kind: "Paid" }) -> fulfill { orderId: pay.orderId items: pay.items }`.

Expected behavior:

- The step pattern MUST be a Uffda Over that binds `items` and `orderId`
  from the step input and MUST ignore extra keys.
- `fulfill` on `-> fulfill { ... }` MUST be that step. The object MUST be
  its input. The step body MUST be the fanout, whose components are
  `Carrier` and `Summarize`.
- `fanout items` MUST map that list. `Summarize(orderId)` MUST set
  Summarize's `orderId` field.
- After two deliveries the run MUST succeed with
  `{ kind: "Summarize", orderId: "o-1", total: 2 }`.
- `-> succeed` is Next; a fanout step MAY `-> Name` instead. `pay.orderId`
  reads the `pay` step's completion.

Postconditions:

- Tests: `tests/grammar.rs` and `tests/interpreter.rs` (`req:workflows-008`).
