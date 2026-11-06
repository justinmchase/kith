---
id: components-001
title: A parent owns declared children; it does not inherit them
spec_ref: .agents/specifications/components.spec.md#composition
---

# Has-a children

## Requirement

Preconditions:

- A program declares a parent component with a `child` slot of another kind.

Expected behavior:

- Constructing the parent MUST create exactly one child instance per declared
  slot.
- The child MUST be reachable from the parent by slot name and MUST NOT be
  reachable as a subtype of the parent.
- The runtime MUST NOT copy or override the child's machine from the parent.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-001`).
