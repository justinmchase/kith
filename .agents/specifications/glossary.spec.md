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
  example `component`, `workflow`, `scene`, `send`, `to`, `fanout`, `reduce`,
  `from`, `then`, `pick`, `else`, `goto`, `run`, `succeed`, `fail`,
  `complete`, `print`).
- A new concept MUST be added here before a chapter relies on it.
- Kith MUST NOT take Coleslaw's domain terms (aggregate, manager, reactor,
  controller, command, event as Coleslaw defines them). Those belong to
  languages above Kith.

## Structure

- **Program**: the set of modules that make up one Kith application.
- **Module**: one Kith source file.
- **Package**: modules published together under one name and version, imported
  as Uffda's packages are.
- **Root**: the instance that has no parent. A running program has one root:
  the top-level instance of its exported scene (a component or a workflow).
- **Scene**: a named instance tree that constructs workflow and component
  kinds, fills ports, and may add unnamed children.
- **Port**: a typed named slot on a component or workflow for one child
  (`Kind name;`). The scene fills it. `send to name` addresses that instance.

## Uffda

- **Pattern**: a Uffda pattern. Messages, fields, and parameters are matched by
  patterns. A value has a pattern when the pattern matches it, and the value
  used is the value the pattern produces.
- **Expression**: a Uffda expression. Expressions compute values and have no
  side effects.

## Components and trees

- **Component**: a named, composable kind with a state machine, optional
  fields, and optional ports. Instances of a component are constructed by a
  scene. A component is not a subtype of another component.
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
  a handler. The keyword is `send`.
- **To**: a route to a named port (`send to`).
- **Up**: a route toward the parent, and then toward the root if not consumed
  (`send up`).
- **Down**: a route toward direct children of a kind (`send down`).
- **Consume**: a matched transition that handles a message so the message does
  not continue on its route.

## Machines

- **State**: a named condition a machine is in.
- **Start state**: the state a machine begins in.
- **Transition**: the handling of one enabling message in one state: matching
  a pattern, computing with expressions, changing machine state, and sending
  messages or spawning work. A transition is the only behavior.
- **Machine state**: the state the machine is in, plus the values of its fields.
- **Goto**: a component statement (`goto Name`) that enters another declared
  state of this instance. It is not a workflow `then`.

## Dispatch

- **Actor**: a unit of concurrent execution with a mailbox. Every component
  instance is an actor. An in-flight transition MAY be an actor.
- **Mailbox**: the ordered queue of messages an actor has not yet taken.
- **Dispatcher**: the runtime that routes messages, schedules ready actors, and
  is the parallelization mechanism. It is not a declaration in the program.
- **Uffda runtime**: the `uffda-runtime` crate: languages, patterns,
  expressions, values. It does not schedule Kith actors.
- **Kith runtime**: the interpreter in the `kith` binary that owns instances
  and turns, and calls the Uffda runtime to match and evaluate.
- **Ready**: an actor that has at least one message in its mailbox, or a
  transition actor whose work can proceed.

## Workflows

- **Workflow**: a flow graph (task, map, choice, pass, run, succeed, fail) in
  the family of AWS Step Functions. It is not a component state machine. The
  graph is a value. A scene MAY run a workflow as its root.
- **Step**: one node in a workflow graph. It is not an actor. Completing it
  follows `then` (or a choice clause).
- **Run**: one execution of a graph, serializable and restorable.
- **Complete**: finishing a component or nested workflow as a work item
  (`complete V`). The payload is the step output.
- **Print**: a transition statement (`print E`) that records a diagnostic
  value. It is not an expression and MUST NOT complete the instance.
- **Child workflow**: a workflow instance owned by another workflow or by a
  component. Ownership is has-a.
- **Fan-out**: a map step (`fanout path -> Kind reduce Reducer from Init`)
  that spawns one work item per list element.
- **Join**: fan-in of those work items. Each completion is reduced into an
  accumulator (`{ kind: "Reduce", acc, item }`), then `then`.
- **Reduce**: the fold that is fan-in. The reducer receives the aggregate
  and one item; its `complete` payload is the next aggregate.
- **Progress**: a tree of branches and leaves as work fans out and back in,
  rolled up into completed and total counts, plus any progress content the
  program reports.
- **Stream**: a sequence of messages that may arrive over the life of an
  instance, not only at its start.
- **Cluster**: a set of processes that may run workflow instances. Which
  process runs an instance is not part of the program's meaning.
