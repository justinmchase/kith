---
id: random-002
title: a seeded Random component fixes a sequence and is not Crypto
spec_ref: .agents/specifications/random.spec.md#seeded-component
---

# Seeded Random

## Requirement

Preconditions:

- A program imports `Random` from `kith:random`.
- A parent holds `rng: Random` and constructs `Random(seed: 1)`.

Expected behavior:

- `"Unit"` MUST reply `"Number" n` with `n` in `[0, 1)`. A second new run
  with seed `1` MUST reply the same first number. Seed `2` MUST reply a
  different first number.
- `"Range" 0 1000000` on `Random(seed: 1)` MUST equal the first
  `(rand 0 1000000)` of a new run.
- `"Uniform" 10 20` MUST reply a number in `[10, 20)`. `"Bernoulli" 0`
  MUST reply `0`. `"Bernoulli" 1` MUST reply `1`. `"Normal" 0 1` MUST
  reply a finite number. The same seed MUST repeat that normal sample.
- `"Nope"` MUST reply `"Error" "unknown"`. `"Uniform" 5 1` MUST reply
  `"Error" "bounds"`. `"Bernoulli" 2` MUST reply `"Error" "probability"`.
  `"Normal" 0 0` MUST reply `"Error" "deviation"`. None of those MUST
  advance the generator: the next `"Unit"` MUST equal the first `"Unit"`
  of a new `Random(seed: 1)`.
- A draw MUST NOT change the next `(rand 0 1000000)` and MUST NOT change
  the bytes of `Crypto` `"Random" 4`.
- Snapshot after one `"Unit"` MUST restore the generator state as decimal
  text, not a list of draws, and the next `"Unit"` MUST equal the second
  `"Unit"` of a new `Random(seed: 1)`.
- `Random()`, `Random(seed: "no")`, `Random(seed: (sub 0 1))`, and
  `Random(seed: (diff_seconds (instant "2024-01-01T00:00:01.500Z") (instant "2024-01-01T00:00:00.000Z")))`
  MUST fail construction with a message containing `seed`. A source
  number is a whole number, so the fraction is that `1.5` second span.
  `Random(seed: 1, seed: 1)` MUST fail with a message containing
  `duplicate`. `Random(seed: 1 extra: 1)` MUST fail with a message
  containing `unknown field`.
- `import "kith:crypto" Random` and `import "kith:random" Crypto` MUST
  fail resolution.

Postconditions:

- Tests: `tests/stdlib.rs` (`req:random-002`).
