---
id: workflows-005
title: Order workflow accepts, charges, and ships line items
spec_ref: .agents/specifications/workflows.spec.md#example
---

# Order workflow

## Requirement

Preconditions:

- The normative Order program is loaded (`examples/order.kith`).

Expected behavior:

- Injecting `{ kind: "Order", orderId: "o-1", items: ["sku-a", "sku-b"], card: "ok" }`
  MUST succeed with two `{ kind: "Shipped", ... }` payloads.
- Injecting `{ kind: "Nope" }` MUST fail the run with `invalid order`.

Postconditions:

- Tests: `tests/interpreter.rs`, `tests/grammar.rs`, and `tests/cli.rs`
  (`req:workflows-005`).
