---
id: components-004
title: Unmatched send up drops at the root; unmatched send down fails
spec_ref: .agents/specifications/components.spec.md#matching; .agents/specifications/components.spec.md#routing
---

# Unmatched send up and send down

## Requirement

Preconditions:

- A child sends up a message the parent does not handle, a parent sends down a
  message a selected child does not handle, or a send down pattern matches no
  child.

Expected behavior:

- An unmatched send up MUST continue to the next ancestor and MUST drop at the
  root with no error.
- An unmatched send down MUST fail the send with a diagnostic naming the child
  and MUST NOT be forwarded to the child's children.
- `send down P V` with zero matching children MUST fail with a diagnostic.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-004`).
