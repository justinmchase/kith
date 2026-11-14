---
id: dispatch-003
title: send after is due on the clock; idle does not fire it; run does
spec_ref: .agents/specifications/timers.spec.md#constraints; .agents/specifications/dispatch.spec.md#component-actors
---

# Delayed send

## Requirement

Preconditions:

- A component executes `send after (rand 1 3) up V` (Order's MockShip).

Expected behavior:

- The sending turn MUST evaluate the delay as seconds and MUST complete
  without delivering `V`.
- `World::run_until_idle` MUST leave the timer pending.
- `World::run` MUST advance the virtual clock and deliver `V` as an up
  message on a later turn.
- Snapshot after idle MUST restore those timers so `run` still delivers.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:dispatch-003`).
