---
id: components-026
title: A field exists only in the states its in clauses name
spec_ref: .agents/specifications/components.spec.md#state-scoped-fields; .agents/specifications/components.spec.md#declaration; .agents/specifications/glossary.spec.md#machines
---

# A field exists only in the states its in clauses name

## Requirement

Preconditions:

- One field has one name. The value stays across the states where it
  exists, drops on the way out of that set, and initializes again on
  re-entry.
- The pattern on the field is the default for every state. A state
  overrides it only inside its `in` clause.
- The first `in` opts the field into only the named states. A blank
  `in Hold` includes Hold on the field pattern. Omit every `in` and the
  field exists in every state.
- `in Open|Hold` is one clause shared by those states. A state with its own
  `pre` or `post` gets its own `in`. There is no semicolon after an `in`.
  The block closes with `}`.
- `field note: number?;` is the whole-component form: every state, writable
  in every state, `pre` and `post` are the field pattern. Semicolon, no
  block.
- `write` on an `in` means handlers in those states may assign the field.
  A state not marked `write` may read it and may leave it. An assignment
  there is rejected. If no `in` is marked `write`, the field is writable in
  every state where it exists. `write` does not change `pre` or `post`.
  The initializer still runs when the field comes into scope, including on
  re-entry.
- `Box` keeps `total` in every state. `note` exists in Open and Hold, those
  handlers may assign it, and leaving either state requires `number`.
  `phase` exists in Open. The test harness delivers each message as `down`.

```kith
component Box {
  field total: number = 0;
  field note: number? = null {
    in Open|Hold write
      post number
  }
  field phase: number = 1 {
    in Open write
  }

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
}

export scene Main { Box(); }
```

- `strict` is its own component. The initializer is matched against
  `number`. `"no"` does not match. Creation fails before the instance runs.

```kith
component Box {
  field strict: number = "no";
  start Idle {}
}

export scene Main { Box(); }
```

- `broken` exists only in Tight, so creation leaves it absent. `@Bad` would
  enter Tight and records `@Tick`. The initializer misses `number`. The
  transition does not commit.

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
  field broken: number = "no" {
    in Tight
  }
  tick: Counter;

  start Idle {
    on down @Bad {
      send to tick @Tick;
      goto Tight;
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

- A handler in a state where `note` does not exist cannot read or write it.
  Each program fails check and says `not in scope`.

```kith
component Box {
  field total: number = 0;
  field note: number = 0 {
    in Open write
  }

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
  field note: number = 0 {
    in Open
  }

  start Idle {
    on down @Read {
      total -> note;
    }
  }

  state Open {}
}

export scene Main { Box(); }
```

- `shown` exists in Idle and in Open. Only Open says `write`, so Idle may
  read `shown` and may leave it, and an assignment in Idle is rejected.

```kith
component Box {
  field shown: number = 1 {
    in Idle
    in Open write
  }

  start Idle {
    on down @Set {
      shown -> 2;
    }
  }

  state Open {}
}

export scene Main { Box(); }
```

Expected behavior:

- The value persists across `goto` among the states the field names. A
  transition that lands outside that set drops it from machine state. The
  value that left is gone. Entering the set again initializes it with the
  init expression, or null when init is omitted, then the field pattern. A
  miss MUST fail that initialization the way a missed field write fails
  (`field {name} did not match`) and the transition MUST NOT commit.
  Nothing recorded is routed.
- A handler outside the set cannot read or write the field. Check MUST fail
  and say `not in scope`.
- An assignment in a state that has the field and is not marked `write`,
  while some other `in` is marked `write`, MUST fail check and say
  `not writable`.
- `complete` does not leave the state, so `post` does not run and a field
  that exists there stays.
- Entering and leaving do not run another handler and do not send.
- At creation, fields that exist in the start state initialize and match
  the field pattern. Fields outside that set are absent. A component field
  whose initializer misses its pattern fails before the instance runs.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-026`). The test contains
  the Box Kith source above. `@Go` delivered as `down` commits `Open`,
  initializes `note` to null and `phase` to `1`, and keeps `total` at `2`.
  `@Bump` commits `Hold` with `note` at `5` and drops `phase`. `@Drop`
  returns to `Idle` and drops `note`. `@Again` enters `Open` again and
  initializes `note` and `phase` once more. `@Wipe` writes `note` to `9`,
  `post` matches `number`, and the write drops with the field. `@Finish`
  completes in `Open`, does not run `post`, and keeps `note`. `@Done`
  completes in `Idle` with `note` absent. Creating the `strict` program
  fails before the instance runs and says `field strict did not match`.
  `broken` is absent at creation. `@Bad` then misses that initialization,
  stays `Idle`, and does not route `@Tick`. The `@Write` and `@Read`
  programs fail check and say `not in scope`. The `@Set` program fails
  check and says `not writable`. Check also rejects a duplicate field name,
  a state named twice, an unknown state, and a rules list that names no
  states.
