# Workflows

This chapter defines workflows: first-class long-running instances, child
workflows, dynamic fan-out and join, streaming inputs, and clustered execution.
They share dispatch with components ([dispatch](./dispatch.spec.md)) and has-a
composition with [scenes](./scenes.spec.md). Terms are defined in the
[glossary](./glossary.spec.md).

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be interpreted
as described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Definitions

- **Workflow declaration**: the source that names a workflow, its fields, and
  its state machine. Static children are constructed by a scene.
- **Work item**: one element of a fan-out, each run as a child instance or a
  transition actor.
- **Join set**: the in-flight work items a join is waiting on.
- **Placement**: which process in the cluster currently runs a workflow
  instance.

## Axioms

- A workflow is a first-class instance: it has identity, a machine, a mailbox,
  and optional children.
- A workflow's identity does not depend on which process currently runs it.
- Fan-out creates children or transition actors dynamically; join enables a
  later transition with their outcomes.
- Progress is a fact about the instance. Any caller that may send the workflow
  a message MAY learn its progress.
- Streaming inputs are messages. They MAY arrive after the workflow has
  started, and they are matched in whatever state the machine is in when they
  arrive.

## Constraints

### Distinct primitive

- Workflow and component are different declarations. A workflow MUST NOT be
  written as a component with extra flags, and a component MUST NOT be written
  as a short-lived workflow.
- A workflow MAY be a child of a component or of another workflow. A component
  MAY be a child of a workflow. Ownership is still has-a.

### Fan-out and join

- Fan-out MUST be able to spawn a number of work items that is known only at
  run time, including zero and one.
- Each work item MUST run as its own actor, so items MAY proceed in parallel.
- Join MUST wait for a declared condition on the join set (every item, a
  pattern of outcomes, or cancellation of the rest) and MUST then enqueue a
  single outcome message on the workflow's component actor.
- A workflow MUST NOT block a dispatcher thread while joining. Join is a
  transition actor.

### Streaming

- A workflow MUST accept messages after it has started, not only a single
  argument at creation.
- Arrival order of a stream is a source of variation when the sender is
  outside the workflow. Within one sender-to-workflow mailbox, order MUST be
  preserved.

### Clustered execution

- A workflow MUST be able to run on any process of the cluster that the
  runtime places it on.
- Placement MUST NOT be part of the program's meaning. A transition MUST NOT
  name a host, thread, or process.
- After a move, the instance MUST resume with the same identity, the same
  machine state, and the same undelivered mailbox messages, as if it had not
  moved.
- Two placements MUST NOT run turns of the same instance at the same time.

### Determinism

- Given the same machine state and the same enabling message, a workflow
  transition MUST be deterministic in the same sense as a component
  transition.
- Which work item of a join set completes first MUST NOT be assumed by the
  program. A transition that needs a particular item MUST match it by
  identity or pattern, not by arrival race.

## Mechanism

### Declaration

The following sketch establishes the shape; token spelling is not frozen.

```text
workflow Import {
  start Ready {
    on (Start items) {
      fanout items -> item;
    }
    on item.Done {
      join;
    }
  }
}

export scene ImportRun {
  Import() {
    item = ProcessItem()
  }
}
```

- `workflow Name { ... }` declares a kind of workflow.
- Static children are wired in a [scene](./scenes.spec.md), not on the
  workflow kind. Fan-out MAY create many instances of a wired kind, each an
  item in a join set.
- `fanout collection -> child` spawns one work item per element as children
  of that kind, and records a join set.
- Completions arrive as messages (`item.Done` in the sketch). A join
  transition reduces the join set and, when the join condition holds, continues
  the workflow.

### Progress

- Each fan-out records branches; each work item is a leaf until it fans out
  further.
- Progress MUST roll up completed and total counts along that tree.
- A workflow MAY include progress content computed by expressions from its
  fields. That content MUST be data a caller can read without taking a turn
  that changes the machine.

### Child workflows

- A child workflow is an ordinary child instance whose kind is a workflow.
- Destroying a parent MUST cancel its child workflows. Cancellation MUST
  propagate to their join sets.
- A child workflow's completion is a message to its parent, like any other
  child.

### Clustered execution

- The runtime MAY move a workflow between processes between turns.
- A move MUST wait until the current turn of that instance is complete.
- Durable facts of a workflow are at least: identity, kind, machine state,
  mailbox, join sets, and child identities. How they are stored is the planned
  `persistence` chapter.
- Streaming senders address the workflow by identity, not by placement. The
  cluster MUST route those messages to the current placement.

## Why this design

- **A second primitive, not a component flag.** Components are a topology for
  local concurrent behavior. Workflows are relocatable, joinable, long-running
  work with progress. Collapsing them now would mix mailbox-as-UI-tree with
  mailbox-as-orchestrator before either is built. They share dispatch so the
  runtime stays one scheduler.
- **Fan-out as spawn, join as message.** A blocking join would stall the
  dispatcher. An implicit promise graph would hide actors. Spawn and complete
  as messages keep checkability: the checker varies completion order at the
  mailbox.
- **Placement is not in the program.** If a transition could name a host,
  clustered execution would leak into business and component code, and a
  checker could not relocate work.

## Open questions

- **Join conditions.** All items, first success, first failure, N-of-M, and
  how failures cancel the rest.
- **Partial results.** Whether a join MAY yield a stream of per-item messages
  as well as a final join message.
- **Exactly-once turns** after a crash, versus at-least-once with idempotent
  transitions.
- **How a stream is declared** (a child slot that receives, a pattern of
  messages, a Uffda async iterable).
- **Who may query progress** without sending a message that the machine must
  match.
- **Timers** for workflow deadlines, in the planned `timers` chapter.
- **Whether Coleslaw's planned workflow chapter** is only a restriction of
  this one. Deferred to Coleslaw.
