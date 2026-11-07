---
id: components-004
title: Unmatched bubble drops at the root; unmatched down fails
spec_ref: .agents/specifications/components.spec.md#matching
---

# Unmatched bubble and down

## Requirement

Preconditions:

- A child bubbles a message the parent does not handle, or a parent downs a
  message the child does not handle.

Expected behavior:

- An unmatched bubble MUST continue to the next ancestor and MUST drop at the
  root with no error.
- An unmatched down MUST fail the send with a diagnostic naming the child and
  MUST NOT be forwarded to the child's children.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-004`).
