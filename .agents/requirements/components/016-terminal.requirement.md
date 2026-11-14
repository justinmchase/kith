---
id: components-016
title: goto or complete is at most one terminal and MUST be last
spec_ref: .agents/specifications/components.spec.md#terminal-statements
---

# Terminal statements

## Requirement

Preconditions:

- A component handler body may assign, print, send, `goto`, and `complete`.

Expected behavior:

- `goto` or `complete` MUST be the last statement if present.
- A handler MUST NOT contain both `goto` and `complete`.
- A handler MUST NOT contain more than one `goto` or more than one
  `complete`.
- `check` MUST reject `goto` then `send`, two `goto`s, and `goto` plus
  `complete`.
- `send` then `goto` MUST be accepted. The later self-send MUST match in
  the state entered by that `goto`.
- Assign and `print` before the terminal MUST run on this turn. Sends
  MUST NOT run the receiver on this turn.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-016`).
