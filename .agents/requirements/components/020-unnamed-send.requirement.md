---
id: components-020
title: An unnamed child sends up on the same route, and send down Kind selects a port child and an unnamed child of that kind together
spec_ref: .agents/specifications/components.spec.md#routing; .agents/specifications/components.spec.md#sending
---

# Unnamed send up and send down

## Requirement

Preconditions:

- A parent has one direct child that fills a port and one unnamed direct
  child. The two children may be the same kind. The parent may also have an
  unnamed child of a different kind.
- The unnamed child has no port name.

Expected behavior:

- `send down Kind` MUST deliver one send to each direct child of that kind.
  A port child of kind `Kind` and an unnamed child of kind `Kind` MUST both
  receive that send. A child of another kind MUST NOT.
- `send to` a port MUST deliver only to the child that fills that port. It
  MUST NOT deliver to an unnamed sibling, and the unnamed child MUST NOT
  gain a port name.
- The unnamed child's `send up` MUST be the same ancestor walk as a port
  child's `send up`. The parent MUST match it with `on up`. There is one
  `send up` route. It MUST NOT be delivered to siblings.

Postconditions:

- Tests: none. `components-003` still covers a port child beside an unnamed
  child of a different kind. The specs audit does not fail when no test
  cites this id.
