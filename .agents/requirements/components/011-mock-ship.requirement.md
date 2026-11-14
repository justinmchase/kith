---
id: components-011
title: Carrier dispatches to MockShip, which send-after (rand 1 3) up Delivered
spec_ref: .agents/specifications/components.spec.md#sending; .agents/specifications/timers.spec.md#mechanism; .agents/specifications/workflows.spec.md#waiting-for-an-outside-message
---

# Mock ship callback

## Requirement

Preconditions:

- `examples/order.kith` declares `MockShip` and `Carrier` with port
  `MockShip ship`. The scene fills `ship = MockShip()` under a Carrier
  prototype.

Expected behavior:

- On `{ kind: "Item", ... }` Carrier MUST `send to ship` a `Dispatch` and
  `goto Shipping`.
- MockShip MUST `print` `InTransit` and
  `send after (rand 1 3) up { kind: "Delivered" }`.
- After `run_until_idle`, each Carrier MUST still be in `Shipping` and MUST
  own a `MockShip` child. The Order run MUST NOT yet succeed.
- `World::run` MUST deliver those timers. Shipping MUST match `Delivered` and
  `complete` `Shipped`.

Postconditions:

- Tests: `tests/grammar.rs` and `tests/interpreter.rs` (`req:components-011`).
