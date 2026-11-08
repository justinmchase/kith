---
id: components-010
title: unnamed start, else as a statement, and complete object shorthand
spec_ref: .agents/specifications/components.spec.md#declaration; .agents/specifications/components.spec.md#sending
---

# Unnamed start and else complete

## Requirement

Preconditions:

- `Summarize` in `examples/order.kith` declares `field total: number = 0;`,
  `start {`, `on item: any`, and
  `else complete { kind: "Summarize" total };`.

Expected behavior:

- The start state MUST be named `Start`.
- The last handler MUST be `else` (pattern `any`) whose body is one
  `complete` whose object has keys `kind` and `total` (`total` shorthand
  for `total: total`).
- After two delivered items the Order run MUST succeed with
  `{ kind: "Summarize", total: 2 }`.

Postconditions:

- Tests: `tests/grammar.rs` and `tests/interpreter.rs` (`req:components-010`).
