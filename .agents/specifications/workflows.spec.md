# Workflows

This chapter defines workflows: the structured machines that orchestrate
components and child workflows. They share the instance runtime and
[dispatch](./dispatch.spec.md) with [components](./components.spec.md), and
static children are still wired by a [scene](./scenes.spec.md). Terms are
defined in the [glossary](./glossary.spec.md).

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be interpreted
as described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Definitions

- **Workflow declaration**: the source that names a workflow, its fields, its
  ports, and its steps.
- **Step**: a named phase of a workflow machine. It is syntax, not a third
  kind of actor.
- **Work item**: one child instance spawned by a fan-out.
- **Join set**: the in-flight work items a `join all` is waiting on.

## Axioms

- A workflow is a first-class instance: identity, fields, ports, mailbox,
  children, and a current step.
- A workflow shares the component runtime. It is not a component with flags.
- Steps dispatch by messages. A step's work items are component or workflow
  instances, never a `step` actor.
- Fan-out creates children at run time; join enables a later transition with
  their completions. Join MUST NOT block a dispatcher thread.
- Progress is a fact about the instance. A caller MAY read completed and total
  counts without taking a turn.

## Constraints

### Distinct primitive

- `workflow` and `component` are different declarations.
- A component MUST NOT declare `step`, `fanout`, `join`, or `goto`.
- A workflow MUST NOT declare `state`. It MUST declare exactly one start step.
- A workflow MAY be a child of a component or of another workflow. A component
  MAY be a child of a workflow. Ownership is still has-a.
- The unique exported [scene](./scenes.spec.md)'s root MUST be a workflow.
  `kith run` and `World::start` run that workflow.

### Fan-out and join

- `fanout expr -> Kind` MUST spawn one work item per element of `expr`,
  including zero and one. `expr` MUST evaluate to a list.
- Each work item MUST be an instance of `Kind`, which MUST be a component or
  workflow with no ports in this slice.
- Each work item MUST receive `{ kind: "Item", item, index }` as a down send
  and MUST run as its own actor.
- `complete V` MUST mark the instance done for join accounting and MUST
  `send up V`. Completing the scene root MUST mark it done and MUST NOT send
  up.
- `join all { ... }` MUST run as a later turn when the join set is empty,
  with `{ kind: "Joined", results }` bindings. It MUST NOT block.
- Which work item completes first MUST NOT be assumed. Results are collected
  in completion order.

### Determinism

- Given the same machine state and the same enabling message, a workflow
  transition MUST be deterministic in the same sense as a component
  transition.

## Mechanism

### Declaration

```kith
export Child;
export Ping;
export Main;

component Child {
  start Idle {
    on ({ kind: "Ping", n: number }) {
      complete { kind: "Pong", n: n };
    }
  }
}

workflow Ping {
  field heard: number = 0;
  Child child;

  start Run {
    on ({ kind: "Ping", n: number }) {
      send to child { kind: "Ping", n: n };
    }
    on ({ kind: "Pong", n: number }) {
      heard -> n;
    }
  }
}

export scene Main {
  Ping() {
    child = Child();
  }
}
```

Fan-out:

```kith
component Worker {
  start Idle {
    on ({ kind: "Item", item: any, index: number }) {
      complete { kind: "Done", item: item, index: index };
    }
  }
}

workflow Import {
  start Fetch {
    on ({ kind: "Start", items: any }) {
      fanout items -> Worker;
    }
    join all {
      goto Done;
    }
  }

  step Done {
    complete { kind: "Imported", results: results };
  }
}

export scene Main {
  Import();
}
```

- `workflow Name { ... }` declares a kind. Instances are created by a scene
  or by fan-out.
- `Kind name;` is a port, filled by the scene, same as a component port. The
  kind MAY be a component or a workflow.
- `start Name { ... }` is the start step. Later steps use `step Name { ... }`.
- Statements at the top of a step run when the step is entered (start or
  `goto`). `on (pattern) { ... }` waits in that step. `join all { ... }` waits
  for the step's join set.
- `goto Name;` enters that step. `fanout expr -> Kind;` records a join set.
- `complete V;` finishes the instance as a work item.

### Progress

- An active join set MUST expose `{ completed, total }` without a turn.
- Nested roll-up and progress content expressions are not required of this
  slice.

### Child workflows

- A child workflow is an ordinary child whose kind is a workflow.
- Completing a child is `complete`, which sends up to the parent like any
  other child.

## Syntax tree

- `{ kind: "program", ..., workflows: [Workflow] }`
- `{ kind: "workflow", name: string, fields: [Field], ports: [Port], steps: [Step] }`
- `{ kind: "step", name: string, start: boolean, enter: [Statement], handlers: [Handler], join: Join | null }`
- `{ kind: "all", body: [Statement] }`
- `{ kind: "fanout", collection: Expression, component: string }`
- `{ kind: "goto", step: string }`
- `{ kind: "complete", message: Expression }`

Exactly one step MUST have `start: true`. `join` MUST be `null` when the step
has no `join all`.

## Example

Injecting `{ kind: "Ping", n: 7 }` at the `Ping` root of scene `Main` MUST
`send to child`, the child MUST `complete` `{ kind: "Pong", n: 7 }`, and
`Ping.heard` MUST become `7`.

Injecting `{ kind: "Start", items: ["a", "b"] }` at `Import` MUST fan out two
`Worker` instances, join both, `goto Done`, and complete with two results.

## Why this design

- **A second primitive, not a component flag.** Components keep local
  protocol. Workflows compose many of those machines. They share dispatch so
  the runtime stays one scheduler.
- **Step is syntax.** A third actor kind would split the dispatcher. Work
  items stay component or workflow instances.
- **Fan-out as spawn, join as a later turn.** A blocking join would stall the
  dispatcher. Completions stay messages, so a checker can vary their order.
- **The world runs a workflow.** The scene still builds the tree; the root of
  that tree is the application flow.

## Open questions

- **Join conditions** other than `join all`: first success, first failure,
  N-of-M, cancellation of the rest.
- **Fan-out of kinds with ports** (a prototype subgraph per item).
- **Exactly-once turns** after a crash.
- **Timers** for workflow deadlines.
- **Clustered execution**, still specified in spirit here and in
  [dispatch](./dispatch.spec.md), is not required of this slice.
- **Whether Coleslaw's planned workflow chapter** is only a restriction of
  this one. Deferred to Coleslaw.
