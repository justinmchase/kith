---
id: workflows-009
title: a component step completion is read by the step name
spec_ref: .agents/specifications/workflows.spec.md#declaration; .agents/specifications/workflows.spec.md#working-value
---

# Step completion

## Requirement

Preconditions:

- A workflow is `pay: ChargeCard -> paid`, then
  `paid: pick { ({ kind: "Paid" quantity }) -> fulfill { accountId: pay.accountId quantity }; else -> decline; }`.
- `ChargeCard` completes `{ kind: "Paid", accountId: "a-1", quantity: 3 }`.
- `fulfill` runs `Fulfill` and that component completes
  `{ kind: "Filled", accountId, quantity }`.

Expected behavior:

- `pay.accountId` MUST read the `ChargeCard` completion from the step named
  `pay`.
- The pick pattern MUST bind `quantity` from that completion.
- The object after `fulfill` MUST be `Fulfill`'s input, not the whole
  completion. The run MUST succeed with `accountId` `"a-1"` and `quantity`
  `3`.

Postconditions:

- Tests: `tests/interpreter.rs` and `tests/grammar.rs` (`req:workflows-009`).
