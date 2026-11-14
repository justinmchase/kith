---
id: components-010
title: unnamed start, else as a statement, and complete object shorthand
spec_ref: .agents/specifications/components.spec.md#declaration; .agents/specifications/components.spec.md#sending
---

# Unnamed start and else complete

## Requirement

Preconditions:

- `Summarize` in `examples/order.kith` declares `field orderId: any;`,
  `field total: number = 0;`, `start {`, `on item: any`, and
  `else complete { kind: "Summarize" orderId total };`.

Expected behavior:

- The start state MUST have implicit name `Start` and MUST NOT be a
  `goto` target (`named: false`).
- The last handler MUST be `else` (pattern `any`) whose body is one
  `complete` whose object has keys `kind`, `orderId`, and `total`
  (shorthand `orderId` / `total` for `orderId: orderId` / `total: total`).
- After two delivered items the Order run MUST succeed with
  `{ kind: "Summarize", orderId: "o-1", total: 2 }`.

Postconditions:

- Tests: `tests/grammar.rs` and `tests/interpreter.rs` (`req:components-010`).
