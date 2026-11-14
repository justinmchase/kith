---
id: workflows-005
title: Order waits for delivery confirmations then summarizes fan-in
spec_ref: .agents/specifications/workflows.spec.md#example; .agents/specifications/workflows.spec.md#waiting-for-an-outside-message
---

# Order workflow

## Requirement

Preconditions:

- The normative Order program is loaded (`examples/order.kith`).

Expected behavior:

- Injecting a paid order with two items MUST accept and charge, then wait on
  two `Carrier` work items. Each Carrier MUST `(writeLine stdout { kind:
  "PickupRequested" … })` and dispatch its `MockShip`. After
  `run_until_idle` the run MUST NOT succeed.
- `World::run` MUST fire MockShip's delayed `Delivered` ups. `Summarize`
  MUST fold each shipment and succeed with
  `{ kind: "Summarize", orderId: "o-1", total: 2 }`.
- Snapshot after pickup and restore MUST still deliver those timers.
- Injecting `{ kind: "Nope" }` MUST fail the run with `invalid order`.

Postconditions:

- Tests: `tests/interpreter.rs`, `tests/grammar.rs`, and `tests/cli.rs`
  (`req:workflows-005`).
