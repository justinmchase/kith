---
id: workflows-005
title: Ping workflow sends to child, child completes Pong, heard becomes 7
spec_ref: .agents/specifications/workflows.spec.md#example
---

# Ping workflow

## Requirement

Preconditions:

- The normative Ping/Child program is loaded, as source or as a hand-built
  syntax tree, including exported scene `Main`.

Expected behavior:

- Injecting `{ kind: "Ping", n: 7 }` at the `Ping` root MUST `send to child`,
  the child MUST `complete` `{ kind: "Pong", n: 7 }`, and `Ping.heard` MUST be
  `7`.

Postconditions:

- Tests: `tests/interpreter.rs` and `tests/cli.rs` (`req:workflows-005`).
