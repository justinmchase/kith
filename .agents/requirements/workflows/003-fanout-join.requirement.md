---
id: workflows-003
title: fanout 0, 1, or N workers reduce into an accumulator
spec_ref: .agents/specifications/workflows.spec.md#definitions; .agents/specifications/workflows.spec.md#declaration
---

# Map fan-out and fan-in

## Requirement

Preconditions:

- A workflow `start fanout items -> Worker reduce Count from 0 then succeed`,
  a port-less `Worker`, and a port-less `Count` that folds with
  `on acc: any item: any { complete (add acc 1); }`.

Expected behavior:

- Zero items MUST succeed with the `from` identity `0` and MUST NOT invoke
  the reducer.
- One or more items MUST each receive `{ kind: "Item", item, index }`. The
  map output MUST be the count of completions, not an array of payloads.
- Fan-out of a kind that needs ports without a prototype MUST fail check.

Postconditions:

- Tests: `tests/interpreter.rs`, `tests/grammar.rs`, and `tests/host.rs`
  (`req:workflows-003`).
