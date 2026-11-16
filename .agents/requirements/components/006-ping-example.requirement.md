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

- Injecting the symbol `Ping` and `7` MUST `send to child`, Child MUST
  `send up` the symbol `Pong` and `7`, and `Ping.heard` MUST be `7`.
  The string `"Ping"` MUST NOT match `on @Ping`.

Postconditions:

- Tests: `tests/interpreter.rs` and `tests/cli.rs` (`req:components-006`).
