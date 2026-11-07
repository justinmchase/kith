---
id: components-008
title: else is the last handler and matches any remaining message
spec_ref: .agents/specifications/components.spec.md#declaration; .agents/specifications/components.spec.md#matching
---

# Else handler

## Requirement

Preconditions:

- `ValidateOrder` in `examples/order.kith` declares a typed `on` and an
  `else` in that order.

Expected behavior:

- The program MUST parse. The `else` handler MUST be the last handler of
  that state and MUST have pattern `any`.
- Injecting `{ kind: "Nope" }` MUST still take that default and fail the
  Order run with `invalid order`.

Postconditions:

- Tests: `tests/grammar.rs` and `tests/interpreter.rs` (`req:components-008`).
