---
id: builtins-005
title: Timer sends @Tick on a millisecond interval without blocking a turn
spec_ref: .agents/specifications/timers.spec.md#timer-component; .agents/specifications/builtins.spec.md#timer-component
---

# Timer component

## Requirement

Preconditions:

- A program imports `Timer` from `kith:time`.
- A parent holds `timer: Timer` and constructs `Timer(interval: 1500)`.
- The parent handles `on up @Tick`.

Expected behavior:

- Construction MUST record `interval` as `1500` and MUST arm a
  dispatcher-clock delay of `1.5` seconds. `World::start` followed by
  `World::run_until_idle` MUST leave `beats` at `0` and the dispatcher clock
  at `0`. The pending timer MUST be due at `1500` milliseconds.
- The constructing turn MUST NOT sleep. The delay is a pending timer.
- `World::fire_next_timer` MUST advance the clock to `1500` and, after the
  following idle, the parent MUST have counted one `@Tick`. A second fire
  MUST advance to `3000` and count a second `@Tick`. Idle between those
  fires MUST NOT count another tick.
- `@Tick` MUST be the symbol `Tick`. A handler `on up "Tick"` MUST NOT
  count it.
- A down message `"Nope"` MUST reply `"Error" "unknown"` and MUST NOT move
  the clock or count a tick. The original delay MUST stay due at `1500`.
- Snapshot after the idle start MUST restore that pending timer so the next
  fire still delivers one `@Tick`.
- `Timer(interval: 0)` and `Timer(interval: (sub 0 5))` MUST fail
  construction with a message containing `positive`. `(sub 0 5)` is the
  expression that yields a negative millisecond count.
- `Timer()` and `Timer(interval: "no")` MUST fail construction with a
  message containing `interval`.
- `Timer(interval: 1500, interval: 1500)` MUST fail construction with a
  message containing `duplicate`.
- `Timer(interval: 1500 extra: 1)` MUST fail construction with a message
  containing `unknown field`.
- `Clock(interval: 1)` MUST fail construction with a message containing
  `does not take construction arguments`.

Postconditions:

- Tests: `tests/builtins.rs` (`req:builtins-005`).
