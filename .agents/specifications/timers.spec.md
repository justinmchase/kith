# Timers

This chapter defines delayed sends: a message scheduled for later without
blocking a turn. Dispatch of those messages is defined in
[dispatch](./dispatch.spec.md). Sending is defined in
[components](./components.spec.md#sending). Host evaluation of `rand` is
defined in [runtime](./runtime.spec.md#host-callables). Terms are defined in
the [glossary](./glossary.spec.md).

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be interpreted
as described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Definitions

- **Clock**: the dispatcher's current time, in seconds. It is virtual. A
  checker MAY replace it.
- **Timer**: a send that is due at a clock time and has not yet been delivered.
- **After**: a delay on a send (`send after Expression`). Expression evaluates
  to seconds.

## Axioms

- A turn MUST NOT sleep, wait on a thread, or otherwise block the dispatcher
  to implement a delay.
- Time is an input to the dispatcher, like the ready queue. Programs do not
  read a wall clock.
- When a timer is due, it becomes an ordinary send: the message is appended to
  a mailbox and taken on a later turn.

## Constraints

- `send after Delay Route V` MUST evaluate `V` and `Delay` on the sending
  turn and MUST NOT deliver `V` until the clock reaches the due time.
- `Delay` is a Uffda expression that produces a number of **seconds**.
  `(rand Lo Hi)` is a Kith host callable: inclusive if both bounds are
  integers, otherwise a number in `[Lo, Hi]`. It is not a Uffda global.
  A checker MAY replace the RNG.
- A negative delay MUST fail that send.
- `World::run_until_idle` MUST drain mailboxes only. Pending timers MUST stay
  pending so a test can observe a wait.
- `World::run` (and `kith run`) MUST advance the clock to the next due timer
  when mailboxes are empty, deliver those sends, and repeat until there is no
  mail and no timer.
- Snapshot MUST include the clock, the RNG, and every pending timer.
  Restore MUST continue those timers.

## Mechanism

```kith
send after (rand 1 3) up { kind: "Delivered" };
send after 2 up { kind: "Delivered" };
send after 5 self "Tick";
```

A `send after Delay self V` is how a component runs later without a child
timer. It MUST evaluate `Delay` and `V` on this turn and MUST NOT block.
The send is delivered when the clock reaches the due time. Recurrence is
rescheduling from the handler, not a `every` keyword. HealthCheck arms
the next Tick from Validating after Check, not from the Tick handler, so
Tick is not taken in Waiting. The later self-send matches a handler whose
route list includes `self`, not `on down` alone.

The first slice uses a virtual clock starting at 0. Advancing to a timer does
not wait on real time.

## Why this design

- **Delay is a send, not a sleep.** Sleeping on a turn would stall every other
  actor on that dispatcher. A timer is just mail with a due time.
- **The delay is an expression.** Bounds belong in source (`(rand 1 3)`), not
  a keyword. Uffda stays pure; Kith evaluates `rand` against World RNG.
- **Idle vs run.** Tests need to see Carrier in `Shipping` with delivery still
  in flight. Operators need `kith run` to finish. Those are two drains of the
  same clock.

## Open questions

- Deadlines that fail a wait if a message does not arrive (`Wait` with a
  timeout).
- Cancellation of a pending self-send when an instance is destroyed.
- Whether source should name the unit (`2s`) instead of bare seconds.
