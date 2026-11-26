---
id: components-026
title: A field may name the states it belongs to
spec_ref: .agents/specifications/components.spec.md#state-scoped-fields; .agents/specifications/components.spec.md#declaration; .agents/specifications/glossary.spec.md#machines
---

# State-scoped fields

## Requirement

Preconditions:

- A field declared on the component is in scope in every state. A field
  declared in a state body is in scope only in that state. Fields in a state
  come before its handlers.
- The same name declared in more than one state is one field. Those
  declarations agree on pattern and init. A component field and a field
  declared in a state do not share a name.
- A constructed field MAY set `states`. That property is not source spelling.
  An empty list or an unknown name fails check.
- `Box` declares `total` on the component. `note` is declared in `Open` and
  in `Hold`. `phase` is declared in `Open` only. The message pattern selects
  the handler. The test harness delivers each message as `down`.

```kith
component Box {
  field total: number = 0;

  start Idle {
    on down @Go {
      total -> 2;
      goto Open;
    }
    on down @Again {
      goto Open;
    }
    on down @Done {
      complete 7;
    }
  }

  state Open {
    field note: number = 0;
    field phase: number = 1;
    on down @Bump {
      note -> 5;
      goto Hold;
    }
    on down @Wipe {
      note -> 9;
      goto Idle;
    }
    on down @Finish {
      complete 3;
    }
  }

  state Hold {
    field note: number = 0;
    on down @Drop {
      goto Idle;
    }
  }
}

export scene Main { Box(); }
```

- `strict` is declared on the component. Its pattern is `number`. `"no"`
  does not match. Creation fails before the instance runs.

```kith
component Box {
  field strict: number = "no";
  start Idle {}
}

export scene Main { Box(); }
```

- `broken` is declared only in `Tight`, so creation leaves it absent. `@Bad`
  would enter `Tight` and records `@Tick`. The initializer misses `number`.
  The transition does not commit.

```kith
component Counter {
  field seen: number = 0;
  start {
    on down @Tick {
      seen -> 1;
    }
  }
}

component Box {
  field total: number = 0;
  tick: Counter;

  start Idle {
    on down @Bad {
      send to tick @Tick;
      goto Tight;
    }
  }

  state Tight {
    field broken: number = "no";
  }
}

export scene Main {
  Box() {
    tick = Counter();
  }
}
```

- A handler in a state that does not declare `note` cannot read or write it.
  `note` is declared in `Open`. Each program fails check and says `not in
  scope`.

```kith
component Box {
  field total: number = 0;

  start Idle {
    on down @Write {
      note -> 1;
    }
  }

  state Open {
    field note: number = 0;
  }
}

export scene Main { Box(); }
```

```kith
component Box {
  field total: number = 0;

  start Idle {
    on down @Read {
      total -> note;
    }
  }

  state Open {
    field note: number = 0;
  }
}

export scene Main { Box(); }
```

Expected behavior:

- The value persists across `goto` among the named states. A transition that
  lands outside the set drops it from machine state. The value that left is
  gone. Entering the set again initializes it with the init expression, or
  null when init is omitted, then the field pattern. A miss MUST fail that
  initialization the way a missed field write fails (`field {name} did not
  match`) and the transition MUST NOT commit. Nothing recorded is routed.
- A handler outside the set cannot read or write the field. Check MUST fail
  and say `not in scope`.
- Entry and post-condition patterns name only fields in scope in that state.
  Check MUST fail if they name a field that is not.
- `complete` does not leave the state, so a scoped field stays.
- Entering and leaving do not run another handler and do not send.
- At creation, fields in scope in the start state initialize, and
  state-scoped fields outside that set are absent, before the entry pattern.
  Creation does not leave a state, so it does not run the start state's
  post-condition.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-026`). The test contains
  the Box Kith source above. `@Go` delivered as `down` commits `Open`,
  initializes `note` to `0` and `phase` to `1`, and keeps `total` at `2`.
  `@Bump` commits `Hold` with `note` at `5` and drops `phase`. `@Drop`
  returns to `Idle` and drops `note`. `@Again` enters `Open` again and
  initializes `note` and `phase` once more. `@Wipe` writes `note` and leaves
  the set, so the write drops. `@Finish` completes in `Open` and keeps
  `note`. `@Done` completes in `Idle` with `note` absent.   Creating the
  `strict` program fails before the instance runs and says `field strict did
  not match`. A field declared only in `Tight` as `field broken: number =
  "no"` is absent at creation. `@Bad` then misses that initialization, stays
  `Idle`, and does not route `@Tick`. The `@Write` and `@Read` programs fail
  check and say `not in scope`. Check also rejects an entry pattern that names a field outside the
  state, a duplicate name, declarations of one name that disagree, an empty
  list, and an unknown state.
