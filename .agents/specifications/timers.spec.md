# Timers

This chapter defines delayed sends: a message scheduled for later without
blocking a turn. Dispatch of those messages is defined in
[dispatch](./dispatch.spec.md). Sending is defined in
[components](./components.spec.md#sending). Terms are defined in the
[glossary](./glossary.spec.md).

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be interpreted
as described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Definitions

- **Clock**: the dispatcher's current time. It is virtual. A checker MAY
  replace it.
- **Timer**: a send that is due at a clock time and has not yet been delivered.
- **After**: a delay on a send (`send after`). The delay is milliseconds on
  the clock, or `random` for a host-chosen delay in that unit.

## Axioms

- A turn MUST NOT sleep, wait on a thread, or otherwise block the dispatcher
  to implement a delay.
- Time is an input to the dispatcher, like the ready queue. Programs do not
  read a wall clock.
- When a timer is due, it becomes an ordinary send: the message is appended to
  a mailbox and taken on a later turn.

## Constraints

- `send after Delay Route V` MUST evaluate `V` on the sending turn and MUST
  NOT deliver it until the clock reaches the due time.
- `Delay` is a millisecond expression, or the keyword `random`. `random` MUST
  pick a positive duration from the host RNG. It is not a Uffda expression.
- `World::run_until_idle` MUST drain mailboxes only. Pending timers MUST stay
  pending so a test can observe a wait.
- `World::run` (and `kith run`) MUST advance the clock to the next due timer
  when mailboxes are empty, deliver those sends, and repeat until there is no
  mail and no timer.
- Snapshot MUST include the clock, the RNG, and every pending timer.
  Restore MUST continue those timers.

## Mechanism

```kith
send after random up { kind: "Delivered" };
send after 25 up { kind: "Delivered" };
```

The first slice uses a virtual clock starting at 0. Advancing to a timer does
not wait on real time.

## Why this design

- **Delay is a send, not a sleep.** Sleeping on a turn would stall every other
  actor on that dispatcher. A timer is just mail with a due time.
- **Idle vs run.** Tests need to see Carrier in `Shipping` with delivery still
  in flight. Operators need `kith run` to finish. Those are two drains of the
  same clock.

## Open questions

- Deadlines that fail a wait if a message does not arrive (`Wait` with a
  timeout).
- Recurring timers and cancellation when an instance is destroyed.
- Whether `random` should take an explicit range in source.
