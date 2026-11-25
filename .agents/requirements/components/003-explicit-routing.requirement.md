---
id: components-003
title: Sends are explicit send to, send down, send up, or send self; unhandled messages do not capture
spec_ref: .agents/specifications/components.spec.md#routing; .agents/specifications/components.spec.md#sending
---

# Explicit routing

## Requirement

Preconditions:

- A scene constructs a parent that declares `child: Child` and fills that
  port, and MAY add unnamed siblings. A message is injected at the parent.

Expected behavior:

- `send to child V` MUST deliver only to the instance that fills port
  `child`.
- `send down V` MUST deliver `V` to every direct child, a port child and an
  unnamed child together. `send down P V` is not a form. The parent does
  not filter children and does not compare component names.
- Each child MUST match `V` with `on down` in its current state, or that
  send fails.
- `send up V` MUST deliver to the parent, not to siblings.
- An unhandled injected message MUST NOT be offered to children.
- In a mixed scene (`child: Child` plus unnamed `Other()`), `send to child V`
  MUST NOT deliver `V` to the unnamed sibling.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-003`).
