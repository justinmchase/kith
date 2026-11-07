---
id: components-004
title: Unmatched send up drops at the root; unmatched to and down fail
spec_ref: .agents/specifications/components.spec.md#matching; .agents/specifications/components.spec.md#routing
---

# Unmatched send up and send down

## Requirement

Preconditions:

- A child sends up a message the parent does not handle, a parent sends to
  or down a message a selected child does not handle, `send to` names a
  missing port, or `send down Kind` matches no child.

Expected behavior:

- An unmatched send up MUST continue to the next ancestor and MUST drop at the
  root with no error.
- An unmatched send to or send down MUST fail the send with a diagnostic
  naming the child and MUST NOT be forwarded to the child's children.
- `send to` a port the instance does not have MUST fail with a diagnostic.
- `send down Kind V` with zero matching children MUST fail with a diagnostic.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-004`).
