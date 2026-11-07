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
  two `ShipItem` workflows. Each Carrier MUST `print` a pickup request.
- `World::send` of `{ kind: "Delivered" }` to each Carrier MUST complete
  those items. `Summarize` MUST succeed with `{ kind: "Summary", total: 2 }`.
- Snapshot after pickup and restore MUST still accept those confirmations.
- Injecting `{ kind: "Nope" }` MUST fail the run with `invalid order`.

Postconditions:

- Tests: `tests/interpreter.rs`, `tests/grammar.rs`, `tests/cli.rs`, and
  `tests/host.rs` (`req:workflows-005`).
