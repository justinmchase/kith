---
id: components-027
title: send to an array field offers the message to each element
spec_ref:
  - .agents/specifications/components.spec.md#routing
  - .agents/specifications/components.spec.md#sending
  - .agents/specifications/components.spec.md#identity-and-lifetime
---

# send to an array field offers the message to each element

## Requirement

Preconditions:

- A component declares `field trees: [Tree*] = []` and may declare a port
  `child: Quiet` filled by the scene, plus an unnamed `Quiet` sibling.
- `trees` is a field value. It is not a port. Storing an instance in the
  array does not add a child and does not fill a port.
- A name is not both a field and a port.
- `send down` does not take a component name. `send up` does not take a
  filter.

Expected behavior:

- `send to trees V` MUST offer `V` to each element, in order. An element
  whose current state has no matching `on down` MUST ignore that offer.
  That MUST NOT fail the send and MUST NOT be forwarded to that element's
  children.
- Zero handlers, including an empty array, MUST succeed.
- `send to child V` MUST still fail with a diagnostic naming that child
  when `child` has no matching handler. The diagnostic contains
  `unmatched down` and the child's kind.
- A field value that is not an array MUST fail the send with a diagnostic
  containing `is not an array`. An element that is not an instance MUST
  fail the send with a diagnostic containing `is not an instance`. The
  turn MUST NOT commit.
- The send MUST NOT construct a child and MUST NOT fill a port. After
  `send to trees`, a `Tree` stored in the field is still not a child.
  The instance MUST stay reachable because the field holds it.
- `send down V` MUST still offer `V` to every direct child and MUST NOT
  fail when a child ignores it. It MUST NOT offer `V` only to the array.
  A direct child that is not an element does not receive `send to trees`.
  An array element that is not a direct child does not receive `send down`.
- `send down Tree V` MUST fail check with a diagnostic containing
  `must not name a component`.
- `send up "Tree" @Season @Summer` MUST NOT deliver `@Season @Summer` to
  the parent. It is one message, not a filter.
- A declaration that uses one name as both a field and a port MUST fail
  check with a diagnostic containing `both a field and a port`.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-027`). The test source
  is Kith. An empty array succeeds. One awake `Tree` counts the offer and
  one asleep `Tree` ignores it. `send to` the quiet port fails unmatched.
  `send down` still reaches both direct children and not the field
  instances. A non-array field and a non-instance element fail the send.
