# Kith specification

The normative contract for Kith, as a set of focused chapters. Each chapter
states what the system guarantees; requirements in
[`../requirements/`](../requirements/README.md) refine chapters into testable
statements.

Kith sits between [Uffda](https://github.com/justinmchase/uffda) and higher-level
languages such as [Coleslaw](https://github.com/justinmchase/coleslaw). Uffda
supplies grammars, patterns, and expressions. Kith supplies components,
workflows, and the actor dispatcher. A language built on Kith MUST use these
primitives rather than redefining them.

## Chapters

- [Overview](./overview.spec.md): what Kith is for, what it is not, its
  principles, and how it sits on Uffda.
- [Glossary](./glossary.spec.md): the vocabulary every other chapter, and the
  language's keywords, use.
- [Components](./components.spec.md): has-a composition, children, state
  machines, and sending messages up or down the tree.
- [Dispatch](./dispatch.spec.md): the runtime that treats components and
  transitions as actors and parallelizes independent work.
- [Workflows](./workflows.spec.md): first-class long-running work, child
  workflows, dynamic fan-out and join, streaming inputs, and clustered
  execution.

## Planned chapters

These are not written yet. They are listed so that new chapters fit the intended
shape.

- `supervision`: what happens when an actor fails, and how a parent restarts or
  stops its children.
- `timers`: deadlines and messages scheduled for later; time from a clock the
  checker can replace.
- `persistence`: what of a durable workflow or component is stored, and how a
  relocated actor resumes.
- `machines`: shared state-machine rules if components and workflows should not
  each carry their own copy.
