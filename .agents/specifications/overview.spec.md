# Overview

This chapter states what Kith is for, what it is not, the principles every other
chapter follows, and how Kith sits on Uffda. Terms are defined in the
[glossary](./glossary.spec.md).

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be interpreted
as described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Purpose

Kith is a runtime layer for programs whose structure is a tree of components
and whose work is messages and state machines. A Kith program
declares components and scenes. The Kith runtime constructs the
exported scene, dispatches messages by matching them as Uffda patterns, runs
enabled transitions, and executes independent actors in parallel.

Kith is built with Uffda: its grammar is a Uffda language, messages are matched
by Uffda patterns, and expressions are Uffda expressions. Higher-level languages
MAY be built on Kith. They MUST use Kith's component, scene, and dispatch
primitives rather than defining their own.

## What Kith is not

- Kith is not Uffda. Uffda is the substrate every language needs: grammars,
  patterns, expressions, matching, and modules. Not every Uffda language wants
  a component tree or actor mailboxes. Those are opt-in,
  so they live here, not in Uffda's core syntax or runtime.
- Kith is not Coleslaw. Coleslaw is a language for business applications. Domain
  vocabulary — aggregate, manager, reactor, controller — belongs there. Kith
  MUST NOT define that vocabulary. How Coleslaw maps its declarations onto
  components is Coleslaw's to specify, after this runtime's
  capabilities are clear.

## Principles

- **Has-a, not is-a.** Composition is the only relationship between components.
  A scene owns the instance tree; a component MUST NOT inherit, override, or
  subtype another component's machine.
- **Messages are the only communication.** Components MUST NOT
  call one another as functions to cause behavior. They send messages. A
  message is routed to a named port or to a field
  (`send to`), down to every direct child
  (`send down`), up toward the root (`send up`), or to this instance
  (`send self`), then matched as a Uffda pattern. A child that does not
  handle a `send down` ignores it. An element of an array field that does not
  handle the offer ignores it, including when the array is empty. A field
  value that is not an array is sent to that one instance.
- **Machines.** Component behavior is a state machine: a matched
  message may send, assign, and `complete`. Actor `send` / `goto` /
  `complete` are statements. Host I/O is Uffda invocation syntax, not a
  second set of keywords.
- **Dispatch is the runtime.** User code MUST NOT schedule actors, wait on
  threads, or lock shared state. Delayed work is `send after`, not a sleep.
  The dispatcher treats components and in-flight transitions as actors and is
  the parallelization mechanism.
- **Patterns match messages.** A receive clause is a Uffda pattern. Checking a
  message against a transition means matching that pattern.
- **Expressions compute values.** Uffda globals (`length`, `add`) and
  expressions in assign, field init, and nested arguments are pure. Host
  stdlib invocations (`(writeLine stdout x)`, `(http_get url)`) have effects;
  they are still expressions, not keywords. Actor effects remain statements:
  sending a message and changing machine state.
- **One tree per exported scene.** A running program is the instance tree of
  its unique exported scene. The root is that scene's top component
  instance. Each instance has at most one parent. The tree is
  has-a all the way down.
- **Scheduling is a source of variation.** The order in which independent actors
  run is not part of a program's meaning. A checker MUST be able to vary that
  order. Everything else a machine does, given the same messages in the same
  order at that actor, MUST be deterministic.

## Stack

```text
Uffda     grammars, patterns, expressions, matching
  ↓
Kith      component, scene, actor dispatch
  ↓
Coleslaw  business applications
```

A language above Kith MAY add declarations, constrain which messages a
declaration may send, and persist what Kith keeps in memory. It MUST NOT
replace matching, dispatch, or the has-a tree with a different computational
model for the same primitives.

## Compilation

- Kith source files MUST use the extension `.kith`.
- Kith's grammar MUST be a Uffda language, declaring the `.kith` extension.
- The `kith` CLI MUST parse, check, compile, and run programs as
  [CLI](./cli.spec.md) specifies. Grammar compilation uses the Uffda CLI;
  program execution uses the Kith runtime on top of `uffda-runtime` (see
  [runtime](./runtime.spec.md)).
- The runtime MUST run compiled syntax trees, not source.
- Compiled trees are build outputs: never edited, never committed, and always
  reproducible from the source.

## Checkability

Kith programs are meant to be checked by running them many times while varying
everything outside a machine's control. The language MUST stay checkable.

- Every source of variation MUST be explicit at an actor's edges: the messages
  that arrive and their order, and the dispatcher's choice among ready actors.
- A machine given the same state and the same enabling message MUST take the
  same transition and send the same messages.
- A feature that would hide a source of variation inside a machine MUST NOT
  be added. The wall is a message to the `Clock` component in
  [built-ins](./builtins.spec.md#time): the checker replaces the driver.
  `(id)` does not read that clock. Its 16-byte value uses the dispatcher
  clock as the timestamp, as [runtime](./runtime.spec.md#identity) defines
  it. A handler that does not evaluate `(id)` MUST NOT mint one. A numeric
  draw is `(rand Lo Hi)` or another draw on that same stream, or a message
  to a seeded `Random` child, as [random](./random.spec.md) defines them.

## Open questions

- **Uffda version.** Library crates are pinned to commit
  `61c06f3eb2dff9f9f399e110754691024b06e468`, not a branch and not `@main`.
  A crates.io release that contains that commit SHOULD replace the pin.
  Coleslaw SHOULD follow Kith's Uffda rather than the other way around.
- **Syntax spelling.** The first-slice declaration form is frozen in
  [components](./components.spec.md#declaration) and
  [scenes](./scenes.spec.md#declaration). Later keywords MUST NOT
  silently change that form.
