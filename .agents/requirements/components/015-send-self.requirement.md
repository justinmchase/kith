---
id: components-015
title: send self matches a route list that includes self; on down does not; a send with no route is invalid
spec_ref: .agents/specifications/components.spec.md#sending; .agents/specifications/components.spec.md#matching
---

# Send self

## Requirement

Preconditions:

- `examples/http.kith` HealthCheck `send self`s Tick and Check, and
  `send after 5 self` the next Tick.

Expected behavior:

- `send self V` MUST be taken on a later turn and MUST match `on self`,
  `on up or self`, or unqualified `on { ... }`.
- `send after Delay self V` MUST be the same self-send, due later.
- `on down` MUST NOT match `send self`. `on self` MUST NOT match inject
  or a send to / down.
- `send V` and `send after Delay V` with no route MUST fail parse.
- In Waiting, `send self` Check then `goto Validating` MUST match Check
  in Validating.

Postconditions:

- Tests: `tests/grammar.rs` and `tests/interpreter.rs` (`req:components-015`).
