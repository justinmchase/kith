---
id: components-003
title: Sends are explicit send up or send down; unhandled messages do not capture
spec_ref: .agents/specifications/components.spec.md#routing; .agents/specifications/components.spec.md#sending
---

# Explicit routing

## Requirement

Preconditions:

- A scene constructs a parent with one or more children. A message is injected
  at the parent.

Expected behavior:

- `send down ({ key: "child" }) V` MUST deliver only to the child in slot
  `child`.
- `send down Child V` and `send down ({ component: "Child" }) V` MUST deliver
  to each direct child of kind `Child`.
- `send down any V` MUST deliver to every direct child.
- `send up V` MUST deliver to the parent, not to siblings.
- An unhandled injected message MUST NOT be offered to children.
- In a mixed scene, `send down { key: "child" } V` MUST NOT deliver `V` to a
  sibling in another slot.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-003`).
