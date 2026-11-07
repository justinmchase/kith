---
id: workflows-003
title: fanout 0, 1, or N workers and join all collects completions
spec_ref: .agents/specifications/workflows.spec.md#fan-out-and-join
---

# Fan-out and join all

## Requirement

Preconditions:

- A workflow `fanout`s a list to a port-less `Worker` component and `join all`.

Expected behavior:

- Zero items MUST join immediately with an empty `results` list.
- One or more items MUST each receive `{ kind: "Item", item, index }`,
  `complete`, and appear in `results`.
- Fan-out of a kind that declares ports MUST fail check.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:workflows-003`).
