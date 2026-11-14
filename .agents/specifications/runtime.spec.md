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
  spawns, or joins happen. Those are transition effects the Kith runtime
  performs after a match, so expressions stay pure.

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
  the body. Sends, spawns, and joins are recorded as Kith effects and handed
  to the dispatcher.
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
- **Effects after the match, not inside it.** If `send up` were a native
  expression, expressions would stop being pure and the dispatcher would be
  hidden in host calls. Walking the transition AST keeps matching and
  scheduling at the layers the [overview](./overview.spec.md#principles)
  names.
- **Own CLI, library Uffda.** Uffda's CLI `compile` lowers Uffda modules
  only; `parse --lang` selects Uffda, pattern, or expression, not an arbitrary
  project language. A `kith` binary that links `uffda-runtime` is the
  supported consumer shape, the same path as Uffda's in-tree consumer crate.

### Host callables

Uffda expressions are pure. Kith MAY intercept a named invocation while
evaluating an expression AST. The first of those is `(rand Lo Hi)`: a number
in that range from World RNG, inclusive when both bounds are integers. It is
not a Uffda global. Snapshot MUST include the RNG so restore is determined.

A host callable MUST return a value on the same turn. It MUST NOT perform
I/O, wait, or send messages. Those are [host I/O](#host-io).

### Host I/O

A host effect that takes time is a later message. It is not a blocked turn
and not an expression. There is no `async` in a handler. `(fetch url)`
MUST NOT be an expression.

The capability is a child kind, the same shape as MockShip. In the program
a mock takes a request **down** and `send after`s a response **up**. A real
host driver for that kind MUST deliver the result on the same route: as
`send up` from that instance. The parent waits in another state and matches
`on up`.

1. A turn `send to`s a request to the child. That turn ends.
2. The dispatcher is not blocked.
3. The mock parks a timer, or the host performs HTTP / disk / a device off
   the dispatcher.
4. A later `send up` carries the result.
5. The parent matches `on up` and may `send up` again or `complete`.

A host driver MUST NOT run Kith `on` handlers for that instance. It is not
a Uffda `HostCallable` (those return on the same turn). How the operator
binds `Http` to curl versus the mock is a later slice. The program shape
is the tree, the messages, and `on down` / `on up`.

The normative program is `examples/http.kith`: a HealthCheck that probes
through Checking, Waiting, and Validating, then waits 5 seconds before the
next Tick. It prints `"Healthy"` when the last three statuses are 200,
`"Unhealthy"` when none of them are 200, and `"Indeterminate"` otherwise.

```kith
component Http {
  start {
    on down "GET" url:any {
      print "Requested" url;
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
      print "Healthy";
      send after 5 self "Tick";
      goto Checking;
    }
    on self "Check" a:(number & not 200) b:(number & not 200) c:(number & not 200) {
      print "Unhealthy";
      send after 5 self "Tick";
      goto Checking;
    }
    else {
      print "Indeterminate";
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
Validating prints, `send after 5 self` Tick, then `goto Checking` so the
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
- **Host I/O adapters.** How a scene-filled `Http` kind is bound to a real
  client process (a native driver registered for that kind).
