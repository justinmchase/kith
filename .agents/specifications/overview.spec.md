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
declares components and workflows. The Kith runtime dispatches messages by
matching them as Uffda patterns, runs enabled transitions, and executes
independent actors in parallel.

Kith is built with Uffda: its grammar is a Uffda language, messages are matched
by Uffda patterns, and expressions are Uffda expressions. Higher-level languages
MAY be built on Kith. They MUST use Kith's component, workflow, and dispatch
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
  A component owns children; it MUST NOT inherit, override, or subtype another
  component's machine.
- **Messages are the only communication.** Components and workflows MUST NOT
  call one another as functions to cause behavior. They send messages. A
  message is routed up toward the root (bubble) or down to children, then
  matched as a Uffda pattern.
- **State machines are the only behavior.** Choosing what happens — which
  messages to send, whether to change state, whether to spawn work — MUST be
  done by a transition of a state machine. Expressions compute values and have
  no effect.
- **Dispatch is the runtime.** User code MUST NOT schedule actors, wait on
  threads, or lock shared state. The dispatcher treats components and
  in-flight transitions as actors and is the parallelization mechanism.
- **Patterns match messages.** A receive clause is a Uffda pattern. Checking a
  message against a transition means matching that pattern.
- **Expressions are pure.** Uffda expressions compute values. Effects are
  explicit: sending a message, changing machine state, spawning or joining
  work.
- **One tree per program root.** A running program is a tree of component and
  workflow instances. Each instance has at most one parent. The tree is has-a
  all the way down.
- **Scheduling is a source of variation.** The order in which independent actors
  run is not part of a program's meaning. A checker MUST be able to vary that
  order. Everything else a machine does, given the same messages in the same
  order at that actor, MUST be deterministic.

## Stack

```text
Uffda     grammars, patterns, expressions, matching
  ↓
Kith      component, workflow, actor dispatch
  ↓
Coleslaw  business applications
```

A language above Kith MAY add declarations, constrain which messages a
declaration may send, and persist what Kith keeps in memory. It MUST NOT
replace matching, dispatch, or the has-a tree with a different computational
model for the same primitives.

## Compilation

- Kith source files MUST use the extension `.kith`.
- Kith's grammar MUST be a Uffda language, declaring the `.kith` extension, so
  Uffda's tools can find and parse Kith source.
- Compiling a program MUST parse each source file with that grammar and write
  its syntax tree according to Uffda's artifact layout.
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

- **Project file.** Whether Kith uses `kith.jsonc` / `kith.lock` with Uffda's
  project shape, or Uffda's `uffda.jsonc` with a language entry. This depends
  on Uffda's support for project files of other languages.
- **Uffda version.** Kith will target a released Uffda. Coleslaw currently pins
  0.9.2; Uffda is on 0.10.x. Kith SHOULD take a current Uffda and let Coleslaw
  follow.
- **Shared machines chapter.** Components and workflows both have state
  machines. Whether their rules should be one chapter, once the two primitives
  have settled.
- **Syntax spelling.** Keywords, punctuation, and how children, fields, and
  handlers are written are not frozen. The chapters state the contract;
  sketches are illustrative until a grammar chapter exists.
