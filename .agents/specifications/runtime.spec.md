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
- **Program pipeline**: a `.kith` source file parsed by that grammar to a
  syntax tree, checked, and interpreted by the Kith runtime.

## Axioms

- Uffda does not know what a component, workflow, actor, or mailbox is.
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
- Program artifacts are Kith syntax trees (and later checked IR). They are
  outputs of `kith compile`. Their format is Kith's. They MUST NOT be Uffda
  module declarations.

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
  rule on the source text, and emits the syntax tree.
- `kith run` walks that tree: it constructs instances, enqueues messages, and
  for each turn calls Uffda to match `on` patterns and evaluate expressions in
  the body. Sends, spawns, and joins are recorded as Kith effects and handed
  to the dispatcher.
- Import of one `.kith` file from another is Kith's module graph. Uffda's
  resolver remains the resolver of the grammar (`*.uff` / `jsr:`), not of
  application programs.

## Why this design

- **Uffda is a matcher, not an actor system.** Its public API already runs
  languages, matches structured values, evaluates expressions, and lets a
  consumer embed its own compiled grammars. That is the whole grammar and
  "did this message match" layer. Adding `Component` to that crate would couple
  Uffda's release train to clustering and mailboxes, and every Uffda language
  would inherit types it does not want.
- **Effects after the match, not inside it.** If `bubble` were a native
  expression, expressions would stop being pure and the dispatcher would be
  hidden in host calls. Walking the transition AST keeps matching and
  scheduling at the layers the [overview](./overview.spec.md#principles)
  names.
- **Own CLI, library Uffda.** Uffda's CLI `compile` lowers Uffda modules
  only; `parse --lang` selects Uffda, pattern, or expression, not an arbitrary
  project language. A `kith` binary that links `uffda-runtime` is the
  supported consumer shape, the same path as Uffda's in-tree consumer crate.

## Open questions

- **Program artifact format.** Codec vs JSON for checked `.kith` trees, and
  whether they live under Uffda-style `outDir/ast/...` paths.
- **`.kith` imports through Uffda's resolver.** Not required. If a later Uffda
  change maps extra extensions to artifacts, Kith MAY use it; it MUST NOT block
  on that.
- **Closed object patterns** for message shapes. That is an Uffda pattern
  gap, not an actor gap; track it upstream if Kith messages need it.
- **Host callables** for later I/O (clock, cluster transport). Those are
  Kith-registered `HostCallable` values driving Uffda's existing host driver,
  not new Uffda runtime elements.
