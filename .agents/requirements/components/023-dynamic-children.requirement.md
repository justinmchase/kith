---
id: components-023
title: A transition may construct an unnamed direct child that inits once
spec_ref: .agents/specifications/components.spec.md#dynamic-children; .agents/specifications/components.spec.md#behavior; .agents/specifications/components.spec.md#identity-and-lifetime; .agents/specifications/dispatch.spec.md#component-actors; .agents/specifications/glossary.spec.md#components-and-trees; .agents/specifications/glossary.spec.md#messages-and-routing; .agents/specifications/overview.spec.md#checkability; .agents/specifications/overview.spec.md#principles; .agents/specifications/components.spec.md#syntax-tree
---

# Dynamic children

## Requirement

Preconditions:

- A transition's handler prefix may record a construction beside assign and
  `send`. The source spelling is open. The tree tag is `construct`.
- Scene-constructed children are still created with the parent, in
  declaration order.

Expected behavior:

- The construction MUST record an additional direct child. The parent's turn
  MUST NOT run it. Expressions MUST NOT construct or release a child.
- The new instance MUST be the same component actor as a scene-constructed
  one: one parent, its own identity, its own mailbox, the start state, and
  init once before any other message to it. Init MUST be the first message
  in the new mailbox. Only `on init` matches it. Sends from the spawning
  turn that route to the new instance MUST append after init.
- Release before that instance runs MUST drop the mailbox, init included.
- The new child MUST be unnamed. It MUST NOT fill or replace a port on the
  parent. Ports on the spawning parent stay the scene's fills.
- If the new kind declares ports, the construction MUST fill them under the
  scene rules: known kind, declared kind, no missing port, no duplicate port
  name. Nested unnamed children follow those same rules.
- A key, when the transition supplies one, MUST be a value that transition
  already computed, or null when it supplies none. The program MUST NOT read
  a runtime-minted identity. The same machine state and the same enabling
  message MUST construct the same children.
- The new child MUST NOT be a transition actor and MUST NOT be a fan-out item.
  The turn commits the construction only when the turn commits.

Postconditions:

- Tests: none. A test that stated this rule would have to implement dynamic
  children in the runtime. That runtime is unchanged. The specs audit warns
  when no test cites this id and does not fail.
