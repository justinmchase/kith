---
id: components-001
title: A parent owns scene-constructed children; it does not inherit them
spec_ref: .agents/specifications/components.spec.md#composition; .agents/specifications/scenes.spec.md#constraints
---

# Has-a children

## Requirement

Preconditions:

- A program's exported scene constructs a parent with a child in a named slot.

Expected behavior:

- Constructing the scene MUST create exactly one child instance per nested
  slot.
- The child MUST be reachable from the parent by slot key and MUST NOT be
  reachable as a subtype of the parent.
- The runtime MUST NOT copy or override the child's machine from the parent.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-001`).
