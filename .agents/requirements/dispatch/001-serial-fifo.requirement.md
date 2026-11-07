---
id: dispatch-001
title: The first-slice dispatcher runs one FIFO ready actor per turn
spec_ref: .agents/specifications/dispatch.spec.md#serial-schedule-first-slice
---

# Serial FIFO turns

## Requirement

Preconditions:

- Two stub actors each have a message in their mailbox.

Expected behavior:

- `run_until_idle` MUST run one turn at a time.
- The first enqueued ready actor MUST run before the second.
- Two turns of the same actor MUST NOT overlap.

Postconditions:

- Tests: `tests/dispatch.rs` (`req:dispatch-001`).
