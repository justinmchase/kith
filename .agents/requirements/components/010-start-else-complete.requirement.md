---
id: components-010
title: unnamed start, else as a statement, and complete object shorthand
spec_ref: .agents/specifications/components.spec.md#declaration; .agents/specifications/components.spec.md#sending
---

# Unnamed start and else complete

## Requirement

Preconditions:

- `Summarize` declares `field orderId: any = "o-1";`,
  `field total: number = 0;`, `start {`, `on down { kind: "Item" }`, and
  `else complete { kind: "Summarize" orderId total };`. It is the exported
  scene root.

Expected behavior:

- The start state MUST have implicit name `Start` and MUST NOT be a
  `goto` target (`named: false`).
- The last handler MUST be `else` (pattern `any`) whose body is one
  `complete` whose object has keys `kind`, `orderId`, and `total`
  (shorthand `orderId` / `total` for `orderId: orderId` / `total: total`).
- One `{ kind: "Item" }` MUST set `total` to `1` and MUST NOT complete.
  A later message that misses that `on` MUST `complete`
  `{ kind: "Summarize", orderId: "o-1", total: 1 }`.

Postconditions:

- Tests: `tests/grammar.rs` and `tests/interpreter.rs` (`req:components-010`).
