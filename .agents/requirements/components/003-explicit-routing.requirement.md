---
id: components-003
title: Sends are explicit send to, send down Pattern V, send up, or send self; unhandled messages do not capture
spec_ref: .agents/specifications/components.spec.md#routing; .agents/specifications/components.spec.md#sending
---

# Explicit routing

## Requirement

Preconditions:

- A scene constructs a parent that declares `child: Child` and fills that
  port, and MAY add unnamed siblings. A message is injected at the parent.
- A selector is a pattern. There is no is-a bind on a component's declared
  kind.

Expected behavior:

- `send to child V` MUST deliver only to the instance that fills port
  `child`.
- `send down Child V` MUST deliver to each direct child that satisfies
  `Child` as a looks-like match, a port child and an unnamed child together
  when both satisfy it. The child does not have to be the declared component
  `Child`. A mock matches when it satisfies the pattern.
- `send down any V` MUST deliver to every direct child. `any` is that
  pattern. The message is the value after the selector. A bare `send down
  Child` with no message is not a form.
- `send up V` MUST deliver to the parent, not to siblings.
- An unhandled injected message MUST NOT be offered to children.
- `send to child V` MUST NOT deliver `V` to an unnamed sibling. `send down
  Other V` selects by the pattern `Other`, not by declared kind.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-003`) and
  `tests/lifetime.rs` (`req:components-020`). `send down Child` delivers to
  each direct child that satisfies `Child`, including a mock. A component
  with a different handler pattern is not selected.
