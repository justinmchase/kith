---
id: random-001
title: standard numeric draws share the World RNG with (rand Lo Hi)
spec_ref: .agents/specifications/random.spec.md#numeric-draws; .agents/specifications/runtime.spec.md#host-callables
---

# Numeric draws

## Requirement

Preconditions:

- A program evaluates `(random)`, `(uniform Lo Hi)`, `(bernoulli p)`, and
  `(normal mean deviation)` without importing them.
- `(rand Lo Hi)` still draws on World RNG.

Expected behavior:

- `(random)` MUST be a number in `[0, 1)`. Two calls in one run MUST be
  able to differ. `(add (random) 0)` MUST equal `(random)` on a new run.
  An argument MUST fail the expression and MUST NOT advance the stream.
- `(uniform 10 20)` MUST be in `[10, 20)`. `(uniform 5 1)` and
  `(uniform 1 1)` MUST fail and MUST NOT advance the stream.
- `(bernoulli 0)` MUST be `0`. `(bernoulli 1)` MUST be `1`.
  `(bernoulli 2)` MUST fail and MUST NOT advance the stream.
- `(normal 0 1)` MUST be a finite number. Two new runs MUST agree.
  `(normal 0 0)` MUST fail and MUST NOT advance the stream. The successful
  call consumes two draws: the `(rand 0 1000000)` after one `(normal 0 1)`
  MUST equal the third `(rand 0 1000000)` of a run that only calls `rand`.
- `(random)` then `(rand 0 1000000)` MUST equal the second
  `(rand 0 1000000)` of a run that only calls `rand`. A failed draw MUST
  leave the next `(rand 0 1000000)` equal to the first of a new run.
- Snapshot after one `(random)` MUST restore so the next `(random)` equals
  the second `(random)` of a new run.
- These calls MUST NOT change the bytes of `Crypto` `"Random" 4`. A
  `Crypto` draw MUST NOT change the next `(rand 0 1000000)`.

Postconditions:

- Tests: `tests/stdlib.rs` (`req:random-001`).
