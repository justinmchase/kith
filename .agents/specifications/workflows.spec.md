# Workflows

This chapter defines workflows as a **flow graph**, in the same family as
Amazon States Language (Task, Map, Choice, Pass, Succeed, Fail), not as a
second component state machine. Source is Kith syntax. The graph is data: it
MAY be declared, built at run time, snapshotted, and rehydrated. Terms are
defined in the [glossary](./glossary.spec.md). Components and `complete` are
defined in [components](./components.spec.md). The scene still names which
graph the world runs ([scenes](./scenes.spec.md)).

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY be interpreted as
described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Definitions

- **Workflow declaration**: source that names a flow graph and compiles to the
  same IR a `run` step MAY execute from a value.
- **Step**: one node in that graph (task, map, choice, pass, run, succeed,
  fail). It is not an actor.
- **Run**: one execution of a graph: current step, working input/output,
  in-flight work, status.
- **Task**: run one component or workflow until it `complete`s. That payload
  is the step output and the next step's input.
- **Map**: dynamic fan-out of a list path; fan-in joins completions in
  completion order into an array output.

## Axioms

- A workflow is not a component. It has no `on` handlers and no `state`.
- Completing a step transitions along `then` (or a choice clause). The working
  value is the output of the step that just finished.
- Fan-out cardinality is known only at run time, including zero and one.
- The graph is a value. A declared `workflow Name` is sugar for a named IR.
  `run path` executes whatever graph is in the working value at `path`.
- A run MUST be serializable. Restoring a snapshot MUST resume the same step,
  working value, join set, and mailboxes.

## Constraints

- The unique exported scene's root MUST be a workflow. `kith run` and
  `World::start` run that graph.
- A workflow MUST have exactly one `start` step.
- A task or map kind MUST exist and MUST NOT require unfilled ports unless a
  scene prototype supplies that tree.
- `complete V` on a work item MUST send `V` to the waiting workflow and count
  down the join set. The last completion advances `then`.
- Choice matches the working value against clause patterns in order. `else`
  MUST be last if present. No match MUST fail the run.
- Snapshot and restore MUST round-trip a run, including in-flight map items.

## Mechanism

### Declaration

```kith
component Child {
  start Idle {
    on ({ kind: "Ping", n: number }) {
      complete { kind: "Pong", n: n };
    }
  }
}

workflow Ping {
  start Child then succeed;
}

export scene Main {
  Ping();
}
```

```kith
workflow Import {
  start fanout items -> Worker then succeed;
}

workflow Host {
  start run plan then succeed;
}

workflow Checkout {
  start Validate: ValidateOrder then Decide;
  Decide: pick {
    { ok: true } -> Pay;
    else -> Reject;
  };
  Pay: ChargeCard then succeed;
  Reject: fail "invalid";
}
```

- `start Body then Target` is the entry node (implicit name `Start` when
  unnamed). `Name: Body then Target` is a later node.
- **task** `Kind` — spawn `Kind`, send the working value, wait for `complete`.
- **map** `fanout path -> Kind` — `path` is a key on the working object.
  Spawn one `Kind` per element with `{ kind: "Item", item, index }`. Output is
  the array of payloads.
- **choice** `pick { Pattern -> Name; else -> Name; }` — like ASL Choice.
- **pass** `pass Expression` — replace the working value (ASL Pass).
- **run** `run path` — execute the workflow IR at that key (dynamic graph).
- **succeed** / **fail Expression** — terminal (ASL Succeed / Fail).
- `then Name` | `then succeed` | `then fail E` is ASL `Next` / `End`.

Scene children under a workflow are **prototypes**, not live children. A task
or map that names that kind clones the prototype tree (ports filled). Otherwise
it spawns a bare instance (no ports allowed).

### Working value

- Inject at the root sets the working value and enters `start`.
- A step's output becomes the next step's input (ASL default ResultPath `$`).
- The run's output is the working value at succeed. `kith run` prints it.

### Dynamic graphs

A workflow IR is tagged data with `kind: "workflow"`, `start`, and `steps`.
Uffda expressions MAY build that value. `run plan` MUST accept it. Building
graphs at run time MUST NOT require new source.

### Snapshot

`World::snapshot` MUST emit JSON for every instance (component machine or
workflow run), join sets, mailboxes, and ready queue. `World::restore` MUST
accept that document plus the program and continue.

## Syntax tree

- `{ kind: "workflow", name, start, steps: [FlowStep] }`
- `{ name, start: boolean, body: StepBody, next: Next | null }`
- `body.kind`: `task` `{ component }`, `map` `{ items, component }`,
  `choice` `{ clauses: [{ pattern, step }] }`, `pass` `{ value }`,
  `run` `{ path }`, `succeed`, `fail` `{ error }`
- `next.kind`: `goto` `{ step }`, `succeed`, `fail` `{ error }`

## Example

Inject `{ kind: "Ping", n: 7 }` at `Ping` MUST run task `Child` and succeed
with `{ kind: "Pong", n: 7 }`.

Inject `{ kind: "Start", items: ["a", "b"] }` at `Import` MUST map two
`Worker`s and succeed with two payloads.

## Why this design

- **ASL's graph, Kith's syntax.** Task / Map / Choice / Pass / Succeed / Fail
  are the right verbs. XML/JSON is not.
- **Components stay machines; workflows stay flows.** `on` is how a task
  finishes. `then` is how the application moves.
- **The graph is data.** That is what makes dynamic steps, runtime-built
  workflows, and rehydration the same interpreter.

## Open questions

- Parallel (ASL Parallel), Wait/timers, Retry/Catch, JSONPath InputPath /
  ResultPath beyond a single object key.
- Nested Map iterators that are full subgraphs, not a single kind.
- Exactly-once after a crash versus at-least-once restore.
