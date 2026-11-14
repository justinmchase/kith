# Runtime

This chapter defines how Kith's runtime sits on Uffda's: what it reuses from
crates.io, what it implements itself, and why components and workflows are not
Uffda runtime types. Dispatch semantics are in [dispatch](./dispatch.spec.md).
The operator surface is in [CLI](./cli.spec.md). Terms are defined in the
[glossary](./glossary.spec.md).

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be interpreted
as described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Definitions

- **Uffda runtime**: the published `uffda-runtime` crate. It parses with Uffda
  languages, matches patterns, evaluates expressions, resolves `.uff` modules,
  and holds values.
- **Kith runtime**: this language's interpreter. It owns the instance tree,
  mailboxes, routing, actor turns, fan-out, join, and clustered placement.
- **Grammar pipeline**: Kith's language, authored as `.uff`, compiled by the
  Uffda CLI to `.uff.ast` artifacts, and embedded in the `kith` binary.
- **Program pipeline**: `.kith` modules parsed by that grammar, with relative
  imports resolved, checked, and interpreted from the unique exported scene.

## Axioms

- Uffda does not know what a component, scene, workflow, actor, or mailbox is.
- Matching a message and evaluating an expression are Uffda's work. Scheduling
  those matches as turns of actors is Kith's work.
- A Kith program is not a Uffda module declaration. Lowering `.kith` source
  with Uffda's `UffdaRuntimeCompiler` is not how programs run.
- Host-language native functions inside expressions MUST NOT be how sends,
  spawns, or `goto` happen. Those are transition statements the Kith runtime
  performs after a match. Host I/O is an intercepted invocation, not a
  Kith keyword.

## Constraints

### Reuse

- The `kith` binary MUST depend on released `uffda-runtime` (and the supporting
  crates it needs, such as `uffda-codec`, `uffda-heap`, and `uffda-build`) from
  crates.io. It MUST NOT fork or vendor Uffda's matcher, evaluator, or value
  heap.
- Grammar sources MUST import Uffda's published tokenizer, pattern, and
  expression grammars rather than copying them.
- Compiling those `.uff` grammar sources MUST use a released `uffda` CLI. The
  `kith` crate MUST embed the resulting artifacts (the `uffda-build` consumer
  path) and MUST NOT enable Uffda's product `embed-languages` feature, which
  is Uffda's own grammars.

### Do not extend Uffda for actors

- Kith MUST NOT require Uffda to add component, workflow, actor, mailbox, or
  cluster types to its runtime.
- Kith MUST NOT encode dispatch as a Uffda pattern over a message stream, or
  as a host future inside expression evaluation. Mailboxes and schedules are
  not pattern input.
- When Uffda is missing a capability that is still matching or values (for
  example closed object patterns), that change belongs in Uffda. When it is
  missing a capability that is instances and turns, that change belongs in
  Kith.

### Two artifacts

- Grammar artifacts are Uffda ModuleDeclarations (`.uff.ast`). They are build
  inputs of the `kith` binary.
- Program artifacts are Kith syntax trees (and later checked IR) encoded as
  Uffda data codec records with the **`.kith.ast`** extension. They are
  outputs of `kith compile`. They MUST NOT be Uffda module declarations, and
  they MUST NOT be JSON.

## Mechanism

```text
.uff grammars  --uffda compile-->  .uff.ast  --uffda-build-->  kith binary
.kith source   --kith parse---->   syntax tree
               --kith check---->   checked program
               --kith run------>   Kith runtime
                                      |  match message:  uffda-runtime Pattern
                                      |  eval expression: uffda-runtime Evaluation
                                      |  values:          uffda-runtime Value
                                      |  turns, mailboxes, tree, cluster: Kith
```

- `kith parse` loads the embedded Kith language, runs it as a Uffda language
  rule on the source text, and emits the syntax tree as one compact Uffda
  data codec record.
- `kith run` walks the unique exported scene: it constructs instances, enqueues messages, and
  for each turn calls Uffda to match `on` patterns and evaluate expressions in
  the body. Sends, spawns, joins, and same-turn stream writes are recorded as
  Kith effects and handed to the dispatcher or the bound stream.
- Import of one `.kith` file from another is Kith's module graph. Relative
  specifiers are resolved by Kith. Uffda's resolver remains the resolver of
  the grammar (`*.uff` / `jsr:`), not of application packages.

## Why this design

- **Uffda is a matcher, not an actor system.** Its public API already runs
  languages, matches structured values, evaluates expressions, and lets a
  consumer embed its own compiled grammars. That is the whole grammar and
  "did this message match" layer. Adding `Component` to that crate would couple
  Uffda's release train to clustering and mailboxes, and every Uffda language
  would inherit types it does not want.
- **Statements for the machine, invocations for the host.** If `send up` were
  a native expression, the dispatcher would be hidden in host calls. Walking
  the transition AST keeps matching and scheduling at the layers the
  [overview](./overview.spec.md#principles) names. Host I/O is still an
  expression: `(writeLine stdout x)` is a callee, not a keyword.
- **Own CLI, library Uffda.** Uffda's CLI `compile` lowers Uffda modules
  only; `parse --lang` selects Uffda, pattern, or expression, not an arbitrary
  project language. A `kith` binary that links `uffda-runtime` is the
  supported consumer shape, the same path as Uffda's in-tree consumer crate.

### Host callables

Kith MAY intercept a named invocation while evaluating an expression AST.
The callee decides whether the result is this turn or a later message.

**Same-turn.** `(rand Lo Hi)` is a number in that range from World RNG,
inclusive when both bounds are integers. Snapshot MUST include the RNG so
restore is determined. `(write stream V…)`, `(writeLine stream V…)`, and
`(print V…)` write to a stream capability and return `undefined` this
turn. `(print V…)` is `(writeLine stdout V…)`. World binds `stdin`,
`stdout`, and `stderr` in handler scope. They are not Uffda globals. An
unknown callee MUST fail evaluation.

**Later-turn.** `(http_get url)` has no value this turn. It is legal only
as a send argument: `send self (http_get url)` starts the request and
delivers the result as that message when the host is done. Several async
arguments in one send wait for all (argument order, not completion order)
and deliver one array. `send after` is evaluate now, deliver later;
async I/O is deliver when the host is done. Combining `send after` with an
async argument MUST fail. Check MUST reject an async callee in assign,
field init, an expression statement, or nested argument position.

A same-turn callable MUST NOT block the dispatcher. A later-turn callable
MUST NOT return on this turn and MUST NOT attach a `{ P -> }` continuation.

### Host I/O

A host effect that takes time is a later message. It is not a blocked turn.
There is no `async` keyword and no match block on a call. `goto` after
`send self (http_get url)` is this turn. The result is matched with `on`
in the state that `goto` entered.

```kith
on down "Go" url:any {
  send self (http_get url);
  goto Fetching;
}

state Fetching {
  on { status: 200 url: any } {
    send up "ok";
    goto Idle;
  }
  else {
    send up "unknown";
    goto Idle;
  }
}
```

Two URLs that must both be 200 are one send of two async args, not two
sends (two messages would strand the second if the first `goto`s away):

```kith
on down "Go" url1:any url2:any {
  send self (http_get url1) (http_get url2);
  goto Fetching;
}

state Fetching {
  on a:any b:any {
    send up "ok";
    goto Idle;
  }
}
```

The mock `(http_get url)` delivers `{ status: 200, url }` after one
virtual second. A real host driver MAY replace that delay with a socket;
the program shape is the send and the `on`. N URLs known in source are N
arguments. N unknown until runtime remains workflow `fanout` plus a
reducer.

A child capability (the same shape as MockShip) MAY still take a request
**down** and `send after` a response **up**. A real host driver for that
kind MUST deliver on that same up route. The parent waits in another
state and matches `on up`.

1. A turn `send`s an async payload, or `send to`s a request to a child.
   That turn ends (`goto` is this turn).
2. The dispatcher is not blocked.
3. The host parks a timer, or performs HTTP / disk / a device off the
   dispatcher.
4. A later message carries the result (the send's route, or `send up`
   from the child).
5. The waiting state matches `on` and may `send up` again or `complete`.

A host driver MUST NOT run Kith `on` handlers for a child instance it
replaces. Same-turn stream writes are not Uffda `HostCallable` returns
that wait; they return `undefined` now. The real driver for a kind this
runtime provides is the built-in import in
[built-ins](./builtins.spec.md#registration). `Pipe` and `Clock` are those
drivers. The scene-filled `Http` in `examples/http.kith` remains the user
mock and still replies with one `"Response"`. `kith:http` is not this slice.

The normative program is `examples/http.kith`: a HealthCheck that probes
through Checking, Waiting, and Validating, then waits 5 seconds before the
next Tick. It `(writeLine stdout …)`s `"Healthy"` when the last three
statuses are 200, `"Unhealthy"` when none of them are 200, and
`"Indeterminate"` otherwise. The scene-filled `Http` child remains the
capability mock for that example; `(http_get url)` is the stdlib spelling
for the same later-message shape without a child.

```kith
component Http {
  start {
    on down "GET" url:any {
      (writeLine stdout "Requested" url);
      send after (rand 1 3) up "Response" 200 "ok";
    }
  }
}

component Client {
  Http http;
  start Idle {
    on down "Go" url:any {
      send to http "GET" url;
      goto Sending;
    }
  }
  state Sending {
    on up "Response" status:number body:any {
      send up "Done" status body;
      goto Idle;
    }
  }
}

component HealthCheck {
  field s1: number?;
  field s2: number?;
  field s3: number?;
  Client client;
  start Checking {
    on init {
      send self "Tick";
    }
    on self "Tick" {
      send to client "Go" "https://example.com";
      goto Waiting;
    }
  }
  state Waiting {
    on up "Done" status:number body:any {
      s1 -> s2;
      s2 -> s3;
      s3 -> status;
      send self "Check" s1 s2 s3;
      goto Validating;
    }
  }
  state Validating {
    on self "Check" a:200 b:200 c:200 {
      (writeLine stdout "Healthy");
      send after 5 self "Tick";
      goto Checking;
    }
    on self "Check" a:(number & not 200) b:(number & not 200) c:(number & not 200) {
      (writeLine stdout "Unhealthy");
      send after 5 self "Tick";
      goto Checking;
    }
    else {
      (writeLine stdout "Indeterminate");
      send after 5 self "Tick";
      goto Checking;
    }
  }
}
```

`Http` MUST NOT `complete` and MUST NOT `goto Sending`. It is a
scene-filled capability: it `send up`s a reply and stays in unnamed
`Start`. Unnamed start is not a `goto` target.

`Client` is the same shape. After `Done` it `send up`s and `goto Idle` so
HealthCheck can `Go` again. It MUST NOT `complete`. Returning to start
requires a named start (`start Idle`).

HealthCheck is the run root. Construction init `send self`s the first Tick;
the operator MUST NOT inject Tick. Checking sends Go and `goto Waiting`.
Waiting shifts the window, `send self` Check, then `goto Validating`.
Validating writes a line, `send after 5 self` Tick, then `goto Checking` so the
next Tick is taken in Checking. Arming Tick from Checking would deliver
it while Waiting if the GET takes longer than the delay. `send after` is
not a wait: the turn continues and the send is due later. `goto` is last.
`World::run` / `kith run` follow that timer forever; tests fire timers one
due-time at a time. Checking, Waiting, and Validating are one start and
two named states; a component MUST NOT declare three `start` states.

## Open questions

- **Artifact layout.** Whether `.kith.ast` files live under Uffda-style
  `outDir/ast/...` paths.
- **`.kith` imports through Uffda's resolver.** Not required. If a later Uffda
  change maps extra extensions to artifacts, Kith MAY use it; it MUST NOT block
  on that.
- **Closed object patterns** for message shapes. That is an Uffda pattern
  gap, not an actor gap; track it upstream if Kith messages need it.
- **Host I/O adapters.** A built-in module binds its driver by import.
  Binding a user-declared `Http` kind to a native client, and `kith:http`
  itself, are not this slice. The example mock stays the user component.
