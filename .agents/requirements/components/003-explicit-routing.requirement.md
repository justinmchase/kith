---
id: components-003
title: Sends are explicit send to, send down Kind, or send up; unhandled messages do not capture
spec_ref: .agents/specifications/components.spec.md#routing; .agents/specifications/components.spec.md#sending
---

# Explicit routing

## Requirement

Preconditions:

- A scene constructs a parent that declares `Child child` and fills that
  port, and MAY add unnamed siblings of another kind. A message is injected
  at the parent.

Expected behavior:

- `send to child V` MUST deliver only to the instance that fills port
  `child`.
- `send down Child V` MUST deliver to each direct child of kind `Child`,
  including a named port whose kind is `Child`.
- `send down any V` MUST deliver to every direct child.
- `send up V` MUST deliver to the parent, not to siblings.
- An unhandled injected message MUST NOT be offered to children.
- In a mixed scene (`Child child` plus unnamed `Other()`), `send to child V`
  MUST NOT deliver `V` to the unnamed sibling, and `send down Other V` MUST
  NOT deliver `V` to the named `Child`.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-003`).
