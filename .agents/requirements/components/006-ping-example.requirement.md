---
id: components-006
title: Child completes Pong; Ping workflow succeeds with n 7
spec_ref: .agents/specifications/components.spec.md#example; .agents/specifications/workflows.spec.md#example
---

# Ping example

## Requirement

Preconditions:

- The normative Ping/Child program is loaded.

Expected behavior:

- Injecting `{ kind: "Ping", n: 7 }` MUST run task `Child`, the child MUST
  `complete` `{ kind: "Pong", n: 7 }`, and the workflow MUST succeed with that
  payload.

Postconditions:

- Tests: `tests/interpreter.rs` and `tests/cli.rs` (`req:components-006`).
