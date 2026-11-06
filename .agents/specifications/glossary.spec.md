# Glossary

This chapter defines Kith's vocabulary. Every chapter, and every keyword in the
language, uses these terms with these meanings. Kith is named for kith, not kin:
the company you keep by acquaintance, not by descent.

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be interpreted
as described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Using the glossary

- A chapter MUST use a term from this glossary only with the meaning given here.
- A keyword that names one of these concepts MUST be the term itself (for
  example `component`, `workflow`, `child`).
- A new concept MUST be added here before a chapter relies on it.
- Kith MUST NOT take Coleslaw's domain terms (aggregate, manager, reactor,
  controller, command, event as Coleslaw defines them). Those belong to
  languages above Kith.

## Structure

- **Program**: the set of modules that make up one Kith application.
- **Module**: one Kith source file.
- **Package**: modules published together under one name and version, imported
  as Uffda's packages are.
- **Root**: the component or workflow instance that has no parent. A running
  program has one root.

## Uffda

- **Pattern**: a Uffda pattern. Messages, fields, and parameters are matched by
  patterns. A value has a pattern when the pattern matches it, and the value
  used is the value the pattern produces.
- **Expression**: a Uffda expression. Expressions compute values and have no
  side effects.

## Components and trees

- **Component**: a named, composable unit with a state machine, optional state,
  and optional children. A component has-a its children; it is not a subtype of
  another component.
- **Instance**: one running component or workflow, with its own identity, state,
  children, and mailbox.
- **Child**: an instance owned by another instance. Ownership is has-a.
- **Parent**: the instance that owns a child. An instance other than the root
  has exactly one parent.
- **Tree**: the has-a ownership graph of a running program. It is a tree: no
  instance has two parents, and no instance is its own ancestor.

## Messages and routing

- **Message**: a value sent from one instance to another, matched as a pattern
  at the receiver. Messages are the only communication that causes behavior.
- **Send**: the act of placing a message on a route. Sending does not wait for
  a handler.
- **Bubble**: a route toward the parent, and then toward the root if not
  consumed.
- **Down**: a route toward children: a named child, or every child.
- **Consume**: a matched transition that handles a message so the message does
  not continue on its route.

## Machines

- **State**: a named condition a machine is in.
- **Start state**: the state a machine begins in.
- **Transition**: the handling of one enabling message in one state: matching
  a pattern, computing with expressions, changing machine state, and sending
  messages or spawning work. A transition is the only behavior.
- **Machine state**: the state the machine is in, plus the values of its fields.

## Dispatch

- **Actor**: a unit of concurrent execution with a mailbox. Every component
  instance is an actor. An in-flight transition MAY be an actor.
- **Mailbox**: the ordered queue of messages an actor has not yet taken.
- **Dispatcher**: the runtime that routes messages, schedules ready actors, and
  is the parallelization mechanism. It is not a declaration in the program.
- **Ready**: an actor that has at least one message in its mailbox, or a
  transition actor whose work can proceed.

## Workflows

- **Workflow**: a first-class long-running instance with a state machine,
  identity independent of the process that currently runs it, optional children,
  and progress.
- **Child workflow**: a workflow instance owned by another workflow or by a
  component. Ownership is has-a.
- **Fan-out**: spawning zero or more child instances from a collection or
  stream, dynamically, as part of a transition.
- **Join**: waiting for fan-out children to complete, then enabling a
  transition with their results.
- **Progress**: a tree of branches and leaves as work fans out and back in,
  rolled up into completed and total counts, plus any progress content the
  program reports.
- **Stream**: a sequence of messages that may arrive over the life of an
  instance, not only at its start.
- **Cluster**: a set of processes that may run workflow instances. Which
  process runs an instance is not part of the program's meaning.
