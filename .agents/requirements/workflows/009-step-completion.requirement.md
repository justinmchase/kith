---
id: workflows-009
title: a component step completion is read by the step name
spec_ref: .agents/specifications/workflows.spec.md#declaration; .agents/specifications/workflows.spec.md#working-value
---

# Step completion

## Requirement

Preconditions:

- A workflow is `pay: ChargeCard -> paid`, then
  `paid: pick { ({ kind: "Paid" quantity }) -> fulfill { accountId: pay.accountId quantity ticket: "t-1" }; else -> decline; }`,
  then `fulfill: Fulfill -> note` and
  `note: { accountId: any quantity: number ticket: any } pass { kind: "Noted", accountId: accountId, quantity: quantity, charged: pay.accountId, ticket: ticket } -> succeed`.
- `ChargeCard` completes `{ kind: "Paid", accountId: "a-1", quantity: 3 }`.
- `Fulfill` completes `{ kind: "Filled", accountId, quantity, ticket }` only
  when its input has `ticket` `"t-1"`.

Expected behavior:

- `fulfill` on the edge MUST be the step whose body is the component
  `Fulfill`. The object after that name MUST be `Fulfill`'s input. That
  message MUST carry `ticket` `"t-1"`, which the `Paid` completion does not.
- In `paid`, `quantity` MUST be bound from that step's input and
  `pay.accountId` MUST read the output of the step `pay`.
- `note` MUST see its own input (`accountId`, `quantity`, `ticket` from
  `Fulfill`'s completion) and the prior completion `pay`. The run MUST
  succeed with `accountId` `"a-1"`, `quantity` `3`, `charged` `"a-1"`, and
  `ticket` `"t-1"`.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:workflows-009`).
