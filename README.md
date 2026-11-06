# Kith

A runtime layer on [Uffda](https://github.com/justinmchase/uffda) for
components, message dispatch, and workflows.
[Coleslaw](https://github.com/justinmchase/coleslaw) is a language on top of
Kith; other Uffda languages may use it too.

Kith is kith, not kin: composition (has-a), not inheritance (is-a). Components
own children, talk only by messages, and change state only through matched
transitions. The dispatcher runs components and in-flight transitions as
actors. Workflows are first-class: child workflows, dynamic fan-out and join,
streaming inputs, and clustered execution.

This repository is specifications first. There is no executable runtime yet.
The contract lives in [`.agents/specifications/`](./.agents/specifications/README.md).

## Stack

```text
Uffda     grammars, patterns, expressions, matching
  ↓
Kith      component, workflow, actor dispatch
  ↓
Coleslaw  business applications (aggregates, managers, …)
```

Source files will use `.kith`. The grammar will be a Uffda language. The
executable name will be `kith`.
