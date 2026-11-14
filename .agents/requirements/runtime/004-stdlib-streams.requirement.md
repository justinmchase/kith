---
id: runtime-004
title: write, writeLine, and print are host functions on stdout and stderr
spec_ref: .agents/specifications/runtime.spec.md#host-callables; .agents/specifications/components.spec.md#handler-expressions; .agents/specifications/cli.spec.md#commands
---

# Stdlib stream functions

## Requirement

Preconditions:

- A handler evaluates `(writeLine stdout V)`, `(write stdout V)`, or
  `(print V)`. World binds `stdin`, `stdout`, and `stderr`.

Expected behavior:

- `stdout` and `stderr` MUST be stream values in handler scope, not
  keywords.
- `(writeLine stdout "Healthy")` MUST record `"Healthy"` on the world.
- `(writeLine stdout "Requested" url)` MUST record the array
  `["Requested", url]`.
- `(print V)` MUST be the same write as `(writeLine stdout V)`.
- An unknown callee MUST fail evaluation.
- `kith run` MUST still print the run-result record on process stdout.
  Stream writes MUST NOT replace that record.

Postconditions:

- Tests: `tests/interpreter.rs` and `tests/cli.rs` (`req:runtime-004`).
