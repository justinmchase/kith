---
id: workflows-005
title: Ping task Child succeeds with Pong n 7
spec_ref: .agents/specifications/workflows.spec.md#example
---

# Ping workflow

## Requirement

Preconditions:

- The normative Ping program is loaded (task `Child` then succeed).

Expected behavior:

- Injecting `{ kind: "Ping", n: 7 }` MUST run `Child` and succeed with
  `{ kind: "Pong", n: 7 }`.

Postconditions:

- Tests: `tests/interpreter.rs` and `tests/cli.rs` (`req:workflows-005`).
