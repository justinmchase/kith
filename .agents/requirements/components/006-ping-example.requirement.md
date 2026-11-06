---
id: components-006
title: Parent downs Ping, child bubbles Pong, heard becomes 7
spec_ref: .agents/specifications/components.spec.md#example
---

# Ping example

## Requirement

Preconditions:

- The normative Parent/Child program is loaded, as source or as a hand-built
  syntax tree.

Expected behavior:

- Injecting `{ kind: "Ping", n: 7 }` at `Parent` MUST down the child, the child
  MUST bubble `{ kind: "Pong", n: 7 }`, and `Parent.heard` MUST be `7`.

Postconditions:

- Tests: `tests/interpreter.rs` and `tests/cli.rs` (`req:components-006`).
