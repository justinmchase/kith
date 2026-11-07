---
id: cli-001
title: kith parse, check, compile, and run the ping program
spec_ref: .agents/specifications/cli.spec.md#commands
---

# Parse check compile run

## Requirement

Preconditions:

- `examples/ping.kith` is the normative Ping/Child program with exported
  scene `Main` whose root is workflow `Ping`.

Expected behavior:

- `kith parse` MUST print a `program` syntax tree with scene `Main` as a
  Uffda data codec record (not JSON) and MUST NOT start actors.
- `kith check` MUST accept the example and MUST reject a workflow with no
  start step, and a program with no exported scene.
- `kith compile` MUST write a `.kith.ast` data codec artifact.
- `kith run examples/ping.kith --message '{"kind":"Ping","n":7}'` MUST print
  a data codec record of `{ kind: "Pong", n: 7 }` and exit 0.

Postconditions:

- Tests: `tests/cli.rs` (`req:cli-001`).
