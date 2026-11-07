---
id: components-003
title: Sends are explicit bubble or down; unhandled messages do not capture
spec_ref: .agents/specifications/components.spec.md#routing
---

# Explicit routing

## Requirement

Preconditions:

- A parent has a child. A message is injected at the parent.

Expected behavior:

- `down child message` MUST deliver only to that child.
- `bubble message` MUST deliver to the parent, not to siblings.
- An unhandled injected message MUST NOT be offered to children.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-003`).
