---
id: components-001
title: A parent owns scene-constructed children; it does not inherit them
spec_ref: .agents/specifications/components.spec.md#composition; .agents/specifications/scenes.spec.md#constraints
---

# Has-a children

## Requirement

Preconditions:

- A program's exported scene constructs a parent that declares a port and
  fills it, and MAY add unnamed children in the same collection.

Expected behavior:

- Constructing the scene MUST create exactly one child instance per nested
  construction, named ports and unnamed children together.
- A named port MUST be reachable with `send to` that port name.
- An unnamed child MUST live in the parent's child collection and MUST NOT
  require a port on the parent kind.
- The child MUST NOT be reachable as a subtype of the parent.
- The runtime MUST NOT copy or override the child's machine from the parent.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-001`).
