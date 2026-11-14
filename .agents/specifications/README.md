# Kith specification

The normative contract for Kith, as a set of focused chapters. Each chapter
states what the system guarantees; requirements in
[`../requirements/`](../requirements/README.md) refine chapters into testable
statements.

Kith sits between [Uffda](https://github.com/justinmchase/uffda) and higher-level
languages such as [Coleslaw](https://github.com/justinmchase/coleslaw). Uffda
supplies grammars, patterns, and expressions. Kith supplies components, scenes,
workflows, and the actor dispatcher. A language built on Kith MUST use these
primitives rather than redefining them.

## Chapters

- [Overview](./overview.spec.md): what Kith is for, what it is not, its
  principles, and how it sits on Uffda.
- [Glossary](./glossary.spec.md): the vocabulary every other chapter, and the
  language's keywords, use.
- [Components](./components.spec.md): kinds, fields, ports, state machines,
  `send to` / `send down` / `send up`, the declaration syntax, and the syntax
  tree.
- [Scenes](./scenes.spec.md): the instance tree, port fills, unnamed children,
  and Uffda-shaped import/export.
- [Dispatch](./dispatch.spec.md): the runtime that treats components and
  transitions as actors and parallelizes independent work.
- [Workflows](./workflows.spec.md): AWS Step Functions-style flow graphs
  (task, map, choice, pass, run, succeed, fail), dynamic graphs, and
  serializable runs.
- [Runtime](./runtime.spec.md): the split between Uffda's matcher and Kith's
  actor interpreter, and how the `kith` binary consumes crates.io.
- [CLI](./cli.spec.md): `parse`, `check`, `compile`, and `run`.
- [Timers](./timers.spec.md): delayed sends (`send after`); time from a clock
  the checker can replace.
- [Built-ins](./builtins.spec.md): the `kith:` prefix and the data, stream,
  and time modules. `Clock` there is the wall read. The dispatcher clock
  stays in Timers.

## Planned chapters

These are not written yet. They are listed so that new chapters fit the intended
shape.

- `supervision`: what happens when an actor fails, and how a parent restarts or
  stops its children.
- `persistence`: what of a durable workflow or component is stored, and how a
  relocated actor resumes.
- `machines`: shared state-machine rules if components and workflows should not
  each carry their own copy.
