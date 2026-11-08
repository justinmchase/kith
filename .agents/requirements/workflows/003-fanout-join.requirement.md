---
id: workflows-003
title: fanout 0, 1, or N workers join into an accumulator component
spec_ref: .agents/specifications/workflows.spec.md#definitions; .agents/specifications/workflows.spec.md#declaration
---

# Map fan-out and fan-in

## Requirement

Preconditions:

- A workflow `start fanout items -> Worker reduce Count then succeed`,
  a port-less `Worker`, and a port-less `Count` with `field total: number = 0`
  that counts each completion with `on item: any` and `else complete total`.

Expected behavior:

- Zero items MUST still start Count (its field init is `0`) and take
  `else` only. Count MUST `complete` `0`.
- One or more items MUST each receive `{ kind: "Item", item, index }`. Count
  MUST receive one payload per completion, then `else`. The map output MUST
  be the count of completions, not an array of payloads.
- Fan-out of a kind that needs ports without a prototype MUST fail check.

Postconditions:

- Tests: `tests/interpreter.rs`, `tests/grammar.rs`, and `tests/host.rs`
  (`req:workflows-003`).
