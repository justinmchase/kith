---
id: components-018
title: Writing a field matches its pattern; a miss fails the write
spec_ref: .agents/specifications/components.spec.md#declaration
---

# Field pattern match

## Requirement

Preconditions:

- A component declares `field name: pattern` with an optional init.
- HealthCheck declares `field s1: number?;` (and `s2`, `s3`).

Expected behavior:

- Construction MUST match each init (or **null** when init is omitted)
  against that field's pattern. `field heard: number = 0` MUST accept
  `0`. `field n: number;` with no init MUST fail (null is not a number).
- `name -> E` MUST evaluate `E`, then match the result against the
  field's pattern, then store it. `s1 -> s2` MUST match `s2`'s value
  against `s1`'s pattern.
- `number?` MUST accept null and a number. Assigning a string to
  `number?` MUST fail.
- `into Acc(orderId)` MUST match each argument against that field's
  pattern.
- An assignment to an undeclared field MUST fail.

Postconditions:

- Tests: `tests/interpreter.rs` and `tests/host.rs` (`req:components-018`).
