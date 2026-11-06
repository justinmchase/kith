---
id: components-005
title: The first matching on pattern in declaration order consumes the message
spec_ref: .agents/specifications/components.spec.md#matching
---

# First matching handler

## Requirement

Preconditions:

- A state declares two `on` handlers, the first of which matches the message.

Expected behavior:

- The runtime MUST run the first matching handler and MUST NOT run later
  handlers for that message.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-005`).
