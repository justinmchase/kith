---
id: components-025
title: A field pre is matched on entry before post
spec_ref: .agents/specifications/components.spec.md#state-conditions; .agents/specifications/components.spec.md#matching; .agents/specifications/components.spec.md#identity-and-lifetime
---

# A field pre is matched on entry before post

## Requirement

Preconditions:

- `pre` is declared on a field, inside an `in` clause. A state does not
  declare `entry`. Omitting `pre` means that state uses the field pattern.
- Handler choice is the route list and the message pattern. `pre` does not
  select a handler.
- Each assign still matches that field's own pattern before `pre` is tried.
- `Latch` keeps `n` in Shut and Open. Shut's clause is blank, so its `pre`
  and `post` are `number`. Open's `pre` is `1`. No `in` is marked `write`.
  One handler matches `@Open`. One handler matches `@Stuck`.
- The test harness delivers `@Open` and `@Stuck` to Latch as `down`.

```kith
component Counter {
  field seen: number = 0;
  start {
    on down @Tick {
      seen -> 1;
    }
  }
}

component Latch {
  field n: number = 0 {
    in Shut
    in Open
      pre 1
  }
  tick: Counter;

  start Shut {
    on down @Open {
      n -> 1;
      send to tick @Tick;
      goto Open;
    }
    on down @Stuck {
      send to tick @Tick;
      goto Open;
    }
  }

  state Open {}
}

export scene Main {
  Latch() {
    tick = Counter();
  }
}
```

Expected behavior:

- Creating an instance MUST match the start state's `pre` for each field
  that exists there, after the initializer has matched the field pattern
  and before init is offered. A miss MUST fail creation with a diagnostic
  that says `pre` and MUST NOT offer init. Creation does not leave a state,
  so it does not run `post`.
- A transition that would leave MUST match the destination's `pre` against
  the fields that would exist there, after the handler's assignments and
  before commit, and before `post` of the state being left. A turn that
  does not leave, including `complete`, MUST NOT run `pre`.
- A miss MUST fail the transition. The diagnostic MUST say `pre` and name
  the state being entered. Machine state MUST stay as it was at the start
  of the turn. Nothing recorded is routed. A `pre` miss MUST NOT report
  `post`.
- When the destination `pre` matches and `post` of the state being left
  does not, the diagnostic MUST say `post` and name that state. The turn
  MUST NOT commit.
- A field write that misses that field's own pattern MUST fail with that
  field diagnostic before `pre` is tried.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-025`). In the Latch
  example, `@Open` delivered as `down` assigns `n` to `1`, Open's `pre`
  matches, commits `Open`, and routes `@Tick`. `@Stuck` delivered as `down`
  leaves `n` at `0`. Open's `pre` misses, so Latch stays `Shut`, `n` stays
  `0`, and `@Tick` is not routed. A missed `pre` leaves fields, machine
  state, completion, and recorded sends uncommitted. Creation fails before
  init when the initial fields miss the start state's `pre`.
