---
id: components-006
title: Parent sends Ping down, child sends Pong up, heard becomes 7
spec_ref: .agents/specifications/components.spec.md#example
---

# Ping example

## Requirement

Preconditions:

- The normative Parent/Child program is loaded, as source or as a hand-built
  syntax tree, including exported scene `Main`.

Expected behavior:

- Injecting `{ kind: "Ping", n: 7 }` at the `Parent` root MUST send down to
  the child in slot `child`, the child MUST send up `{ kind: "Pong", n: 7 }`,
  and `Parent.heard` MUST be `7`.

Postconditions:

- Tests: `tests/interpreter.rs` and `tests/cli.rs` (`req:components-006`).
