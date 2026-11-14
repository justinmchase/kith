---
id: runtime-001
title: (rand Lo Hi) is a Kith host callable evaluated against World RNG
spec_ref: .agents/specifications/runtime.spec.md#host-callables; .agents/specifications/timers.spec.md#constraints
---

# Rand host callable

## Requirement

Preconditions:

- An expression AST is an invocation of `rand` with two arguments (Order's
  MockShip uses `(rand 1 3)` as a `send after` delay).

Expected behavior:

- Evaluating that invocation MUST return a number from World RNG. When both
  bounds are integers it MUST be inclusive of `Lo` and `Hi`.
- It MUST NOT be resolved as a Uffda global. Snapshot MUST include the RNG
  so a restored run continues with the same sequence.

Postconditions:

- Tests: `tests/grammar.rs` and `tests/interpreter.rs` (`req:runtime-001`).
