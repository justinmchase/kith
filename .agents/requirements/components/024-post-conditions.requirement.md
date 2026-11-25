---
id: components-024
title: A post-condition must match before leaving the state
spec_ref: .agents/specifications/components.spec.md#state-conditions; .agents/specifications/components.spec.md#matching; .agents/specifications/components.spec.md#identity-and-lifetime
---

# Post-conditions

## Requirement

Preconditions:

- A state may declare `post`, a pattern over the fields in scope. The source
  spelling stays open. Omitting `post` means the state declares none.
- Handler choice is the route list and the message pattern. The
  post-condition does not select a handler.
- `Gate` keeps `n` in scope. Idle's `post` is `{ n: 0 }`. Open's `post` is
  `{ n: 1 }`. The `post` line below is that tree property, not a chosen
  source spelling. The example does not leave Open, so Open's post does not
  run on the entering turn.
- Idle's `post { n: 0 }` runs because the turn would leave Idle. It sees the
  fields after the leaving handler's assignments and before `goto` commits.
- The test harness delivers `@Open` and `@Stuck` to Gate as `down`. Gate
  keeps `on down` for those messages.

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
  field n: number = 0;
  tick: Counter;

  start Idle {
    post { n: 0 }
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

  state Open {
    post { n: 1 }
  }
}

export scene Main {
  Gate() {
    tick = Counter();
  }
}
```

Expected behavior:

- On a turn that would leave the state, that state's post-condition MUST
  match the fields of the state being left, after the leaving handler's
  assignments and before `goto` commits. A state that does not declare a
  post-condition passes that check. Only then does the turn commit machine
  state or route recorded sends and constructions.
- A miss MUST fail the transition with a diagnostic that says
  `post-condition` and names the state being left. Machine state MUST stay
  as it was at the start of the turn, including every field the handler
  assigned. Nothing recorded is routed. A construction recorded in that
  turn MUST NOT be created, and its init MUST NOT run.
- A post-condition on a state the turn only enters MUST NOT run on that
  entering turn.
- A turn that does not leave its state MUST NOT run that state's
  post-condition. An assignment that would miss it still commits.
- `complete` does not leave the state, so it MUST NOT run the
  post-condition. The completion commits.
- The pattern is pure. It does not send, construct, or read a clock.
- Creation does not leave a state, so it MUST NOT run the start state's
  post-condition. Init MUST still be offered.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-024`). In the Gate example,
  `@Open` delivered as `down` leaves Idle with `n` still `0`, commits
  `Open`, and routes `@Tick`. `@Stuck` delivered as `down` assigns `n` to
  `1` and then would leave Idle. Idle's `post { n: 0 }` misses, so Gate
  stays `Idle`, `n` stays `0`, and `@Tick` is not routed. A leave whose
  assignments still match the current post commits the goto. A turn that
  stays, a `complete`, and a creation do not run `post`.
