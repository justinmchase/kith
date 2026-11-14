# Kith

A runtime layer on [Uffda](https://github.com/justinmchase/uffda) for
components, message dispatch, and workflows.
[Coleslaw](https://github.com/justinmchase/coleslaw) is a language on top of
Kith; other Uffda languages may use it too.

Kith is kith, not kin: composition (has-a), not inheritance (is-a). A scene
builds the instance tree; components talk only by messages and change state
only through matched transitions. The dispatcher runs components and in-flight
transitions as actors. Workflows are first-class: child workflows, dynamic
fan-out and join, streaming inputs, and clustered execution.

The contract lives in [`.agents/specifications/`](./.agents/specifications/README.md).
This repository also ships a `kith` CLI that parses `.kith` source, checks it,
and runs component programs on `uffda-runtime`.

## Stack

```text
Uffda     grammars, patterns, expressions, matching
  ↓
Kith      component, scene, workflow, actor dispatch
  ↓
Coleslaw  business applications (aggregates, managers, …)
```

Source files will use `.kith`. The grammar will be a Uffda language, compiled
with the released `uffda` CLI and embedded via `uffda-build`. The `kith`
binary will link `uffda-runtime` from crates.io for parse, match, and
expression eval; components, workflows, and actor dispatch stay in Kith.
Uffda does not need new runtime types for those primitives.

```sh
kith parse app.kith
kith check app.kith
kith compile app.kith
kith run app.kith --message '["Ping", 7]'
```
