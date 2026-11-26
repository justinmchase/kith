---
id: components-026
title: A field may name the states it belongs to
spec_ref: .agents/specifications/components.spec.md#state-scoped-fields; .agents/specifications/components.spec.md#declaration; .agents/specifications/glossary.spec.md#machines
---

# State-scoped fields

## Requirement

Preconditions:

- A field with `states` omitted is in scope in every state. A field may set
  `states` to the state names it belongs to. The source spelling stays open.
  Omitting `states` means every state.
- One name is one field. A component-scoped field and a state-scoped field
  do not share a name.
- The list names at least one state of this component. An empty list or an
  unknown name fails check.
- `Box` keeps `total` in every state. `note` names `Open` and `Hold`.
  `phase` names `Open`. `strict` names `Tight`. Those names are the tree
  property `states` on the field node, not a chosen source spelling. The
  message pattern selects the handler. The test harness delivers each
  message as `down`.

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
  field note: number = 0;
  field phase: number = 1;
  field strict: number = "no";
  tick: Counter;

  start Idle {
    on down @Go {
      total -> 2;
      goto Open;
    }
    on down @Again {
      goto Open;
    }
    on down @Bad {
      send to tick @Tick;
      goto Tight;
    }
    on down @Done {
      complete 7;
    }
  }

  state Open {
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
    on down @Drop {
      goto Idle;
    }
  }

  state Tight {}
}

export scene Main {
  Box() {
    tick = Counter();
  }
}
```

- A handler outside the set cannot read or write `note`. `note` names
  `Open`. Each program fails check and says `not in scope`.

```kith
component Box {
  field total: number = 0;
  field note: number = 0;

  start Idle {
    on down @Write {
      note -> 1;
    }
  }

  state Open {}
}

export scene Main { Box(); }
```

```kith
component Box {
  field total: number = 0;
  field note: number = 0;

  start Idle {
    on down @Read {
      total -> note;
    }
  }

  state Open {}
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
  `note`. `@Done` completes in `Idle` with `note` absent. `@Bad` misses
  `strict`'s initialization, stays `Idle`, and does not route `@Tick`. The
  `@Write` and `@Read` programs fail check and say `not in scope`. Check
  also rejects an entry pattern that names a field outside the state, a
  duplicate name, an empty list, and an unknown state.
