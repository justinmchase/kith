---
id: components-019
title: Handler expression statements discard; assign keeps a same-turn value; async is only a send payload
spec_ref: .agents/specifications/components.spec.md#handler-expressions; .agents/specifications/components.spec.md#terminal-statements
---

# Handler expression returns

## Requirement

Preconditions:

- A handler body may contain `E;`, `name -> E`, and `send Route E`.

Expected behavior:

- `(writeLine stdout "hi");` MUST run this turn, record `"hi"`, and
  discard the return (`undefined`). The next statement MUST run this turn.
- `total -> (add total 1);` MUST bind the same-turn value to the field.
- `check` MUST reject `heard -> (writeLine stdout n);`,
  `field heard: any = (writeLine stdout n);`, and
  `heard -> (http_get url);`.
- `check` MUST reject `(http_get url);` as an expression statement.
- There is no `{ P -> }` block on an invocation.

Postconditions:

- Tests: `tests/grammar.rs` and `tests/interpreter.rs` (`req:components-019`).
