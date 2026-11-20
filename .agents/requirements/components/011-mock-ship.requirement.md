---
id: components-011
title: Carrier dispatches to MockShip, which send-after (rand 1 3) up Delivered
spec_ref: .agents/specifications/components.spec.md#sending; .agents/specifications/timers.spec.md#mechanism
---

# Mock ship callback

## Requirement

Preconditions:

- `MockShip` and `Carrier` are components. Carrier has port
  `ship: MockShip`. The exported scene root is Carrier with
  `ship = MockShip()`.

Expected behavior:

- On `{ kind: "Item", ... }` Carrier MUST `send to ship` a `Dispatch` and
  `goto Shipping`.
- MockShip MUST `(writeLine stdout { kind: "InTransit" … })` and
  `send after (rand 1 3) up { kind: "Delivered" }`.
- After `run_until_idle`, Carrier MUST still be in `Shipping` and MUST
  own a `MockShip` child. It MUST NOT be completed yet.
- `World::run` MUST deliver those timers. Shipping MUST match `Delivered` and
  `complete` `Shipped`.

Postconditions:

- Tests: `tests/grammar.rs` and `tests/interpreter.rs` (`req:components-011`).
