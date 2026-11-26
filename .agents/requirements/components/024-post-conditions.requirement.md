---
id: components-024
title: A field post must match before the turn leaves
spec_ref: .agents/specifications/components.spec.md#state-conditions; .agents/specifications/components.spec.md#matching; .agents/specifications/components.spec.md#identity-and-lifetime
---

# A field post must match before the turn leaves

## Requirement

Preconditions:

- `post` is declared on a field, inside an `in` clause. A state does not
  declare `post`.
- Handler choice is the route list and the message pattern. `post` does not
  select a handler.
- `Gate` keeps `n` in Closed and Open. Closed's `post` is `0`. Open's `post`
  is `1`. No `in` is marked `write`. The example does not leave Open, so
  Open's `post` does not run on the entering turn.
- The test harness delivers `@Open` and `@Stuck` to Gate as `down`.

```kith
component Counter {
  field seen: number = 0;
  start {
    on down @Tick {
      seen -> 1;
    }
  }
}

component Gate {
  field n: number = 0 {
    in Closed
      post 0
    in Open
      post 1
  }
  tick: Counter;

  start Closed {
    on down @Open {
      send to tick @Tick;
      goto Open;
    }
    on down @Stuck {
      n -> 1;
      send to tick @Tick;
      goto Open;
    }
  }

  state Open {}
}

export scene Main {
  Gate() {
    tick = Counter();
  }
}
```

Expected behavior:

- On a turn that would leave the state, `post` of each field that exists in
  the state being left MUST match that field's value after the leaving
  handler's assignments and before `goto` commits. An omitted `post` uses
  that clause's `pre`, or the field pattern when `pre` is omitted too.
  Only then does the turn commit machine state or route recorded sends and
  constructions.
- A miss MUST fail the transition with a diagnostic that says `post` and
  names the state being left. Machine state MUST stay as it was at the
  start of the turn, including every field the handler assigned. Nothing
  recorded is routed. A construction recorded in that turn MUST NOT be
  created, and its init MUST NOT run.
- `post` on a state the turn only enters MUST NOT run on that entering turn.
- A turn that does not leave its state MUST NOT run `post`. An assignment
  on that turn still commits.
- `complete` does not leave the state, so it MUST NOT run `post`. The
  completion commits.
- The pattern is pure. It does not send, construct, or read a clock.
- Creation does not leave a state, so it MUST NOT run `post`. Init MUST
  still be offered.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-024`). In the Gate example,
  `@Open` delivered as `down` leaves Closed with `n` still `0`, commits
  `Open`, and routes `@Tick`. `@Stuck` delivered as `down` assigns `n` to
  `1` and then would leave Closed. Closed's `post` misses, so Gate stays
  `Closed`, `n` stays `0`, and `@Tick` is not routed. A turn that stays, a
  `complete`, and a creation do not run `post`.
