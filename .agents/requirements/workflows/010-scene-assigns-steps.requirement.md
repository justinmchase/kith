---
id: workflows-010
title: a scene assigns every workflow step that is not inline
spec_ref: .agents/specifications/workflows.spec.md#declaration; .agents/specifications/workflows.spec.md#constraints; .agents/specifications/scenes.spec.md#constraints
---

# Scene assignment

## Requirement

Preconditions:

- `examples/order.kith` assigns `validate = ValidateOrder()` and
  `pay = ChargeCard()` under `Order`, and leaves `Carrier()` unnamed.
- A workflow step `pay: ChargeCard -> succeed` whose scene is `Sale()` with
  no children.
- A scene assigns `stock = ChargeCard()` where `stock` is `pick`.

Expected behavior:

- A task step MUST be assigned in the scene by its step name and its kind.
  `pay: ChargeCard` is that step. `pick`, `fanout`, `pass`, `run`,
  `succeed`, and `fail` are inline and MUST NOT be assigned.
- Check MUST fail when `pay` is missing. The diagnostic MUST say that the
  step must be assigned.
- Check MUST fail when the assignment's kind is not the step body, and when
  an inline step is assigned.
- With `pay = ChargeCard()`, the step MUST run that component. `ChargeCard`
  completing `{ kind: "Paid", accountId: "a-1" }` MUST be the run output.

Postconditions:

- Tests: `tests/grammar.rs` and `tests/interpreter.rs` (`req:workflows-010`).
