---
id: cli-001
title: kith parse, check, compile, and run the ping program
spec_ref: .agents/specifications/cli.spec.md#commands
---

# Parse check compile run

## Requirement

Preconditions:

- `examples/ping.kith` is the normative Parent/Child program with exported
  scene `Main`.

Expected behavior:

- `kith parse` MUST print a `program` syntax tree with scene `Main` and MUST
  NOT start actors.
- `kith check` MUST accept the example and MUST reject a component with no
  start state.
- `kith compile` MUST write a program artifact.
- `kith run examples/ping.kith --message '{"kind":"Ping","n":7}'` MUST leave
  `Parent.heard` at `7` and exit 0.

Postconditions:

- Tests: `tests/cli.rs` (`req:cli-001`).
