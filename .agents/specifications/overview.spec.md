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
and whose work is messages, state machines, and workflows. A Kith program
declares components, scenes, and workflows. The Kith runtime constructs the
exported scene, dispatches messages by matching them as Uffda patterns, runs
enabled transitions, and executes independent actors in parallel.

Kith is built with Uffda: its grammar is a Uffda language, messages are matched
by Uffda patterns, and expressions are Uffda expressions. Higher-level languages
MAY be built on Kith. They MUST use Kith's component, scene, workflow, and dispatch
primitives rather than defining their own.

## What Kith is not

- Kith is not Uffda. Uffda is the substrate every language needs: grammars,
  patterns, expressions, matching, and modules. Not every Uffda language wants
  a component tree, actor mailboxes, or clustered workflows. Those are opt-in,
  so they live here, not in Uffda's core syntax or runtime.
- Kith is not Coleslaw. Coleslaw is a language for business applications. Domain
  vocabulary — aggregate, manager, reactor, controller — belongs there. Kith
  MUST NOT define that vocabulary. How Coleslaw maps its declarations onto
  components and workflows is Coleslaw's to specify, after this runtime's
  capabilities are clear.

## Principles

- **Has-a, not is-a.** Composition is the only relationship between components.
  A scene owns the instance tree; a component MUST NOT inherit, override, or
  subtype another component's machine.
- **Messages are the only communication.** Components and workflows MUST NOT
  call one another as functions to cause behavior. They send messages. A
  message is routed to a named port (`send to`), down to children of a kind
  (`send down`), or up toward the root (`send up`), then matched as a Uffda
  pattern.
- **Machines and flows.** Component behavior is a state machine: a matched
  message may send, assign, and `complete`. Workflow behavior is a flow graph:
  completing a step follows `then` (or a `pick` clause) with that step's
  output as the working value. Expressions compute values and have no effect.
- **Dispatch is the runtime.** User code MUST NOT schedule actors, wait on
  threads, or lock shared state. The dispatcher treats components and
  in-flight transitions as actors and is the parallelization mechanism.
- **Patterns match messages.** A receive clause is a Uffda pattern. Checking a
  message against a transition means matching that pattern.
- **Expressions are pure.** Uffda expressions compute values. Effects are
  explicit: sending a message, changing machine state, spawning or joining
  work.
- **One tree per exported scene.** A running program is the instance tree of
  its unique exported scene. The root MUST be a workflow. Each instance has at
  most one parent. The tree is has-a all the way down.
- **Scheduling is a source of variation.** The order in which independent actors
  run is not part of a program's meaning. A checker MUST be able to vary that
  order. Everything else a machine does, given the same messages in the same
  order at that actor, MUST be deterministic.

## Stack

```text
Uffda     grammars, patterns, expressions, matching
  ↓
Kith      component, scene, workflow, actor dispatch
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
  that arrive and their order, the completions of child work, what a clustered
  placement does, and the dispatcher's choice among ready actors.
- A machine given the same state and the same enabling message MUST take the
  same transition and send the same messages.
- A feature that would hide a source of variation inside a machine, such as
  reading a wall clock or generating an identity other than through a declared
  service of a higher language, MUST NOT be added here.

## Open questions

- **Uffda version.** Kith will target a released Uffda from crates.io (`uffda-runtime`
  0.10.x or later). Coleslaw currently pins 0.9.2 JSR APIs; it SHOULD follow
  Kith's Uffda rather than the other way around.
- **Shared machines chapter.** Components and workflows both have state
  machines. Whether their rules should be one chapter, once the two primitives
  have settled.
- **Syntax spelling.** The first-slice declaration form is frozen in
  [components](./components.spec.md#declaration) and
  [scenes](./scenes.spec.md#declaration). Later keywords MUST NOT
  silently change that form.
