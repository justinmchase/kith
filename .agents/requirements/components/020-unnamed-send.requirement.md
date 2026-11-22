---
id: components-020
title: An unnamed child sends up on the same route, and one send down selects a port child and an unnamed child that both satisfy the pattern
spec_ref: .agents/specifications/components.spec.md#routing; .agents/specifications/components.spec.md#sending
---

# Unnamed send up and send down

## Requirement

Preconditions:

- A parent has one direct child that fills a port and one unnamed direct
  child. Either child matches when it satisfies the selector pattern. Neither
  match requires the child's declared kind to equal a named component.
- The unnamed child has no port name.

Expected behavior:

- `send down Pattern V` MUST deliver one send to each direct child the
  pattern selects. A port child and an unnamed child that both satisfy the
  pattern MUST both receive that send. A component name in the pattern is a
  looks-like match, including a mock. The message is `V`. A bare `send down
  Kind` with no message is not a form.
- `send down any V` MUST deliver to every direct child.
- `send to` a port MUST deliver only to the child that fills that port. It
  MUST NOT deliver to an unnamed sibling, and the unnamed child MUST NOT
  gain a port name.
- The unnamed child's `send up` MUST be the same ancestor walk as a port
  child's `send up`. The parent MUST match it with `on up`. There is one
  `send up` route. It MUST NOT be delivered to siblings.

Postconditions:

- Tests: none. A test of looks-like selection would have to implement that
  match in the runtime. That runtime is unchanged. The specs audit does not
  fail when no test cites this id.
