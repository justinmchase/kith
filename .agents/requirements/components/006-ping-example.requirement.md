---
id: components-006
title: Ping sends to Child; heard becomes 7
spec_ref: .agents/specifications/components.spec.md#example
---

# Ping example

## Requirement

Preconditions:

- The normative Ping/Child program is loaded.

Expected behavior:

- Injecting `["Ping", 7]` MUST `send to child`, Child MUST
  `send up` `["Pong", 7]`, and `Ping.heard` MUST be `7`.

Postconditions:

- Tests: `tests/interpreter.rs` and `tests/cli.rs` (`req:components-006`).
