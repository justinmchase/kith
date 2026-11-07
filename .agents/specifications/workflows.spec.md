# Workflows

This chapter defines workflows as a **flow graph** in the same family as
[AWS Step Functions](https://docs.aws.amazon.com/step-functions/) (Task, Map,
Choice, Pass, Succeed, Fail — Amazon States Language's verbs, not its JSON).
Source is Kith syntax. The graph is data: it MAY be declared, built at run
time, snapshotted, and rehydrated. Terms are defined in the
[glossary](./glossary.spec.md). Components and `complete` are defined in
[components](./components.spec.md). A scene MAY run a workflow or a
component ([scenes](./scenes.spec.md)). The parent/child machine example is
[Ping](./components.spec.md#declaration).

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
- **Map**: dynamic fan-out of a list path. Fan-in is a **reduce**: each
  completion is folded into an accumulator with the previous aggregate and
  that one item, not collected into an array.

## Axioms

- A workflow is not a component. It has no `on` handlers and no `state`.
- Completing a step transitions along `then` (or a choice clause). The working
  value is the output of the step that just finished.
- Fan-out cardinality is known only at run time, including zero and one.
  Fan-in is a reduce: the working value after a map is the accumulator, not
  a collection of item payloads.
- The graph is a value. A declared `workflow Name` is sugar for a named IR.
  `run path` executes whatever graph is in the working value at `path`.
- A run MUST be serializable as a Uffda data codec record. Restoring a
  snapshot MUST resume the same step, working value, join set, and mailboxes.

## Constraints

- A scene root MAY be a workflow or a component. `kith run` and `World::start`
  run that instance. When the root is a workflow, inject starts the run.
- A workflow MUST have exactly one `start` step.
- A task or map kind MUST exist and MUST NOT require unfilled ports unless a
  scene prototype supplies that tree.
- `complete V` on a work item MUST send `V` to the waiting workflow and count
  down the join set. For a map, `V` MUST be folded one item at a time. The
  last fold (or the `from` identity when the list is empty) advances `then`.
  The join MUST NOT materialize an array of all payloads as the step output.
- Choice matches the working value against clause patterns in order. `else`
  MUST be last if present. No match MUST fail the run.
- Snapshot and restore MUST round-trip a run, including in-flight map items.

## Mechanism

### Declaration

The normative workflow is `examples/order.kith`: validate, choose, charge,
choose, map `Carrier` work items, reduce into `Summarize`. Tasks `complete`;
the graph moves with `then` / `pick`. A map item MAY wait without completing
until the host sends a later message into that work item. The reducer matches
the accumulator then the item, not a tagged envelope.

```kith
component Summarize {
  start Idle {
    on { total } item: any {
      complete { total: (add total 1) };
    }
  }
}

workflow Order {
  start Validate: ValidateOrder then Stock;
  Stock: pick {
    ({ kind: "Accepted" }) -> Pay;
    else -> Reject;
  };
  Pay: ChargeCard then Paid;
  Paid: pick {
    ({ kind: "Paid" }) -> Fulfill;
    else -> Decline;
  };
  Fulfill: fanout items -> Carrier reduce Summarize from { total: 0 } then succeed;
  Reject: fail "invalid order";
  Decline: fail "payment declined";
}
```

```kith
workflow Import {
  start fanout items -> Worker reduce Count from 0 then succeed;
}

workflow Host {
  start run plan then succeed;
}
```

- `start Body then Target` is the entry node (implicit name `Start` when
  unnamed). `Name: Body then Target` is a later node.
- **task** `Kind` — spawn `Kind`, send the working value, wait for `complete`
  (Step Functions Task).
- **map** `fanout path -> Kind reduce Reducer from Init` — `path` is a key on
  the working object. Spawn one `Kind` per element with
  `{ kind: "Item", item, index }` (Step Functions Map). Fan-in is a fold, not
  an array. `Init` is the accumulator for zero items. Each completion, in
  completion order, invokes `Reducer` once: its `on Acc Item` matches Acc
  against the current accumulator and Item against that one payload. The
  reducer's `complete` payload becomes the next `acc`. Only that accumulator
  is retained.
- **choice** `pick { (Pattern) -> Name; else -> Name; }` — Step Functions
  Choice. The pattern is grouped so `->` is Kith's edge, not a Uffda Then.
- **pass** `pass Expression` — replace the working value (Pass).
- **run** `run path` — execute the workflow IR at that key (dynamic graph).
- **succeed** / **fail Expression** — terminal (Succeed / Fail).
- `then Name` | `then succeed` | `then fail E` is Next / End.

Scene children under a workflow are **prototypes**, not live children. A task
or map that names that kind clones the prototype tree (ports filled). Otherwise
it spawns a bare instance (no ports allowed).

### Working value

- Inject at the root sets the working value and enters `start`.
- A step's output becomes the next step's input (Step Functions default
  ResultPath `$`).
- The run's output is the working value at succeed. `kith run` prints it.

### Waiting for an outside message

A task or map item that does not `complete` leaves the run at that step. That
is the callback pattern: Carrier takes `Item` in `Waiting`, `goto Shipping`,
freeze with `World::snapshot`, later `World::restore` and `World::send` a
`Delivered` confirmation into the waiting component.
The workflow graph has no `on`; the message is matched by the work item. The
join set advances when that item `complete`s.

### Dynamic graphs

A workflow IR is tagged data with `kind: "workflow"`, `start`, and `steps`.
Uffda expressions MAY build that value. `run plan` MUST accept it. Building
graphs at run time MUST NOT require new source.

### Snapshot

`World::snapshot` MUST emit one Uffda data codec record for every instance
(component machine or workflow run), join sets, mailboxes, and ready queue.
`World::restore` MUST accept that record plus the program and continue. The
record MUST NOT be JSON.

## Syntax tree

- `{ kind: "workflow", name, start, steps: [FlowStep] }`
- `{ name, start: boolean, body: StepBody, next: Next | null }`
- `body.kind`: `task` `{ component }`, `map` `{ items, component, reduce, from }`,
  `choice` `{ clauses: [{ pattern, step }] }`, `pass` `{ value }`,
  `run` `{ path }`, `succeed`, `fail` `{ error }`
- `next.kind`: `goto` `{ step }`, `succeed`, `fail` `{ error }`

## Example

Inject `{ kind: "Order", orderId: "o-1", items: ["sku-a", "sku-b"], card: "ok" }`
at `Order` MUST accept and charge, then wait on two `Carrier` work items.
After each Carrier receives `{ kind: "Delivered" }`, `Summarize`
MUST fold each `Shipped` payload into `{ total: 2 }`.

Inject `{ kind: "Nope" }` MUST fail the run with `invalid order`.

Inject `{ kind: "Start", items: ["a", "b"] }` at `Import` MUST map two
`Worker`s and reduce them with `Count` to `2`.

## Why this design

- **Step Functions' graph, Kith's syntax.** Task / Map / Choice / Pass /
  Succeed / Fail are the right verbs. JSON state machines are not.
- **Components stay machines; workflows stay flows.** Ping is has-a
  (`send to` / `send up`). Order is a graph (`then` / `pick` / `fanout`).
- **The graph is data.** That is what makes dynamic steps, runtime-built
  workflows, and rehydration the same interpreter.
- **Fan-in is a reduce, not a collect.** A results array is O(n) memory and
  forces the next step to re-walk every payload. `(acc, item) -> acc'` keeps
  a constant-size aggregate. Collecting into a list is still possible: write
  a reducer that concatenates, and accept the cost.

## Open questions

- Parallel, Wait/timers, Retry/Catch, JSONPath InputPath / ResultPath beyond a
  single object key.
- Nested Map iterators that are full subgraphs, not a single kind.
- Exactly-once after a crash versus at-least-once restore.
- Bounded map concurrency so a pending fold queue cannot grow with n.
