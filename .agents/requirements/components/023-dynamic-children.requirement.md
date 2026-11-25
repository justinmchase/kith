---
id: components-023
title: A transition constructs an unnamed child that inits once before other messages
spec_ref: .agents/specifications/components.spec.md#dynamic-children; .agents/specifications/components.spec.md#identity-and-lifetime; .agents/specifications/dispatch.spec.md#component-actors
---

# Dynamic children

## Requirement

Preconditions:

- A transition's handler contains a construct statement. The tree is
  `{ kind: "construct", component: string, key: Expression | null, children: [Instance] }`.
  The source spelling is open.
- The new child does not fill or replace a port on the parent. Scene-constructed
  children are still created with the parent.
- A port of the new child is filled by an instance of the component that
  port declares.

Expected behavior:

- The statement MUST record the construction. The parent's turn MUST NOT run
  the new instance.
- The new instance MUST be a component actor with one parent, its own
  mailbox, and the start state. Init MUST be offered once, and it MUST be
  the first message in that mailbox. Sends from the spawning turn MUST
  append after that init. `on init` is the only match for init.
- The new child MUST be unnamed. `send to` MUST NOT address it. `send down
  V` offers `V` to it together with every other direct child. A child with
  no matching `on down` ignores that offer. That is not a failed send.
- A key, when the transition supplies one, MUST be a value that transition
  already computed. Omitting it MUST mean null. A release MAY match that
  key. The program MUST NOT read a runtime-minted identity.
- If the new component declares ports, the construction MUST fill each one.
  A missing port, a duplicate port name, or a fill whose component is not
  the declared component MUST fail. Nested children are created with the
  new instance.
- Release before that instance runs MUST drop it, init included, so it does
  not run. Expressions MUST NOT construct.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-023`). A construct creates
  an unnamed child, inits it before a same-turn send down, stores the
  supplied key for a later release, rejects a port fill of a different
  component, and drops the child when the same turn releases it before it
  runs.
