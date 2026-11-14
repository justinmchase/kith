---
id: components-009
title: goto enters another declared state; unknown names fail check
spec_ref: .agents/specifications/components.spec.md#declaration
---

# Goto state

## Requirement

Preconditions:

- `Carrier` in `examples/order.kith` starts in `Waiting` and declares
  `state Shipping`.

Expected behavior:

- After `{ kind: "Item", ... }`, Carrier MUST be in `Shipping`.
  `{ kind: "Delivered" }` MUST match there and `complete` `Shipped`.
- `{ kind: "Delivered" }` while still `Waiting` MUST fail as unmatched down.
- `goto` of a name that is not a **named** state of the component MUST
  fail `check`. Unnamed `start { }` is not a target: `goto Start` MUST
  fail unless the start was declared `start Start`.

Postconditions:

- Tests: `tests/interpreter.rs` and `tests/grammar.rs` (`req:components-009`).
