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
  fail). It is not an actor. It has an input message. A step that finishes
  with an output stores that message under the step name. The name on `->`
  is this node. The component, or other pattern, is the body of
  `name: Pattern`.
- **Run**: one execution of a graph: current step, that step's input, the
  outputs of finished steps, in-flight work, status.
- **Task**: run one component or workflow until it `complete`s. The step's
  input is the message sent to it. The completion value is the step output.
- **Map**: dynamic fan-out of a list path. Fan-in is a **reduce**: one
  reducer instance holds the aggregate in its fields. Each completion is
  one item; end of the map takes `else` and `complete`.

## Axioms

- A workflow is not a component. It has no `on` handlers and no `state`.
- Completing a step transitions along `->` (or a choice clause). With no
  input object, the next step's input is the output of the step that just
  finished. `-> Name { ... }` replaces that input with the message the
  object evaluates to. `Name` is a step. A task, map, pass, or run stores
  its output under its step name. A choice does not.
- Expressions in a step's body see that step's input and the outputs of
  earlier steps, each addressed by its step name. The input object on an
  edge that leaves a task, map, pass, or run sees that step's output and
  those same prior completions. A component handler sees the message the
  step sent.
- Fan-out cardinality is known only at run time, including zero and one.
  Fan-in is a reduce: the working value after a map is the reducer's
  `complete` payload, not a collection of item payloads.
- The graph is a value. A declared `workflow Name` is sugar for a named IR.
  `run path` executes whatever graph is in the working value at `path`.
- A run MUST be serializable as a Uffda data codec record. Restoring a
  snapshot MUST resume the same step, working value, step completions,
  join set, and mailboxes.

## Constraints

- A scene root MAY be a workflow or a component. `kith run` and `World::start`
  run that instance. When the root is a workflow, inject starts the run.
- A workflow MUST have exactly one `start` step.
- A task or map kind MUST exist and MUST NOT require unfilled ports unless a
  scene prototype supplies that tree.
- `complete V` on a work item MUST send `V` to the waiting workflow and count
  down the join set. For a map, `V` MUST be sent to the reducer as one item.
  The reducer's `complete` after `else` (including when the list is empty)
  advances the `->` edge. The join MUST NOT materialize an array of all
  payloads as the step output.
- Choice matches the step's input against clause patterns in order. `else`
  MUST be last if present. No match MUST fail the run.
- Snapshot and restore MUST round-trip a run, including in-flight map items.

## Mechanism

### Declaration

The normative workflow is `examples/order.kith`: validate, choose, charge,
choose, map `Carrier` work items, reduce into `Summarize`. Tasks `complete`;
the graph moves with `->` / `pick`. A map item MAY wait without completing
until the host sends a later message into that work item. Fan-out names the
work item and the accumulator component; that component holds the aggregate
in its fields. A step MAY destructure the working value before the body.

```kith
component Summarize {
  field orderId: any;
  field total: number = 0;
  start {
    on item: any {
      total -> (add total 1);
    }
    else complete { kind: "Summarize" orderId total };
  }
}

workflow Order {
  start validate: ValidateOrder -> stock;
  stock: pick {
    ({ kind: "Accepted" }) -> pay;
    else -> reject;
  };
  pay: ChargeCard -> paid;
  paid: pick {
    ({ kind: "Paid" }) -> fulfill { orderId: pay.orderId items: pay.items };
    else -> decline;
  };
  fulfill: { items: any orderId: any } fanout items with Carrier into Summarize(orderId) -> succeed;
  reject: fail "invalid order";
  decline: fail "payment declined";
}
```

```kith
workflow Import {
  start count: { items: any } fanout items with Worker into Count -> succeed;
}

workflow Host {
  start host: run plan -> succeed;
}
```

A step is `name: Pattern`, then one edge form, the same shape as a child
`name: Kind`. The name is a binding (`validate`, `pay`), not the component.
`Validate: ValidateOrder` and `ValidateOrder:` are not steps: the colon is
after the name, and the pattern is what follows it.

```kith
pay: ChargeCard -> paid;
paid: pick {
  ({ kind: "Paid" quantity }) -> fulfill { accountId: pay.accountId quantity };
  else -> decline;
};
```

`pay` is the step name. `ChargeCard` is the body of `pay: ChargeCard`. When
that component completes, the completion is `pay`'s output. `pay.accountId`
reads a field of it. The pick pattern binds fields of `paid`'s input
(`quantity`). `fulfill` on the edge is the step declared as
`fulfill: Pattern`, and the object is that step's input. Input, output, and
what a step may read are defined in [Working value](#working-value).

- `start name: Pattern Edge` is the entry step. `name: Pattern Edge` is a
  later step. Every step has a name.
- The pattern is a task kind (`ChargeCard`), or a destructure (an Over, a
  grouped `(P)`, or a capture) before `fanout`, `pass`, or `run`.
  `{ items: any, orderId: any }` is a Uffda Over: those keys bind, nested
  patterns destructure, extra keys are ignored unless rest requires them.
  No match MUST fail the run.
- After the pattern, the only edge forms are `-> Name`, `-> Name { ... }`,
  `pick { ... }`, `fanout ...`, `pass ...`, `run ...`, `succeed`, and
  `fail E`. `->` is the only edge token. `then` is not a workflow edge.
  `pick` is exclusive: the step has no further `->`, and a clause MUST NOT
  inline a fanout. A clause routes to a step that MAY be a fanout.
- **task** `name: Kind -> Name` — spawn `Kind`, send this step's input, wait
  for `complete` (Step Functions Task). The completion is this step's output
  and is kept under `name`.
- **map** `name: Pattern fanout Expression with Kind into Acc(Init) -> Name`
  — Expression is the list (Step Functions Map). The pattern MAY be omitted
  when the list expression needs no destructure. Spawn one `Kind` per
  element with `{ kind: "Item", item, index }`. `into Acc` is the
  accumulator component. `Acc(orderId)` initializes fields from those
  arguments; field inits on Acc still apply. Each completion is sent as the
  payload (`on item: any`). After the last item (or immediately when the
  list is empty) the runtime ends the accumulator; that takes `else`.
  `Acc`'s `else` MUST `complete`; that payload is the map output.
- **choice** `name: pick { (Pattern) -> Name; (Pattern) -> Name { ... }; else -> Name; }`
  — Step Functions Choice. Each clause names a later step. The pattern is
  grouped so `->` is Kith's edge, not a Uffda Then. A bare field
  (`quantity`) binds that field of this step's input. A choice has no
  output of its own.
- **pass** `pass Expression` — replace the working value (Pass).
- **run** `run path` — execute the workflow IR at that key (dynamic graph).
- **succeed** / **fail Expression** — terminal (Succeed / Fail).
- `-> Name` | `-> Name { ... }` | `-> succeed` | `-> fail E` is Next / End.
  `-> succeed` is one Next, not the only Next; `-> packed` continues the
  graph. `Name` is a step name. The object is that step's input message.
  Its fields are expressions, evaluated in the scope of the step that is
  leaving. Absent the object, the next input is the step output.

Scene children under a workflow are **prototypes**, not live children. A task
or map that names that kind clones the prototype tree (ports filled). Otherwise
it spawns a bare instance (no ports allowed).

### Working value

Each step has an input message. A task, map, pass, or run also has an output
message, stored under that step's name for the rest of the run. A choice,
`succeed`, and `fail` are not stored under the step name.

In the Order graph the choice arm

```kith
({ kind: "Paid" }) -> fulfill { orderId: pay.orderId items: pay.items };
```

names the step `fulfill`. That step is declared as

```kith
fulfill: { items: any orderId: any } fanout items with Carrier into Summarize(orderId) -> succeed;
```

The component, or other pattern, is the body of `fulfill: Pattern`. This
body is a map: it spawns `Carrier` for each item and reduces into
`Summarize`. `Summarize(orderId)` initializes that accumulator's fields.
The object after `fulfill` is the message that becomes `fulfill`'s input.
That edge does not construct a component.

**Into a step.** `-> Name` passes the previous step's output as `Name`'s
input. `-> Name { ... }` evaluates the object as a message, and that message
is `Name`'s input. Inject at the root is the `start` step's input. A task
sends its input to the component and waits for `complete`. A map matches its
pattern against its input, sends one item message per element, and
initializes the accumulator from the `into` arguments. A choice matches its
input against the clause patterns.

**Out of a step.** A component step's output is the value that component
`complete`s. Later steps read it by the step name (`pay.accountId`). A map's
output is the accumulator's `complete` payload. A `pass` output is its
expression. A destructure followed only by `->` keeps the input. A choice selects the
next step and that step's input. `succeed` ends the run with its input.
`fail` ends the run with the error. The output is stored under the step name
before an outgoing input object is evaluated, so that object may read the
step that just finished.

**In scope at a step.** An expression in the step's body sees two sources:

- the run's prior step completions, addressed by step name
- that step's own input: the fields its pattern binds, or, when it has no
  pattern, the fields of the input message

A name bound from the input is the binding the expression sees when that
name is also a step name. A message that is not an object has no field
bindings; it is still the input a task sends or a pass keeps. A component
handler does not see this scope. It sees the message the step sent.

A choice clause binds fields of that choice step's input. Those bindings and
the prior completions are in scope for the clause's input object. In
`({ kind: "Paid" quantity }) -> fulfill { accountId: pay.accountId quantity }`,
`quantity` is bound from `paid`'s input and `pay.accountId` is the output of
the step `pay`.

An input object on the edge that leaves a task, map, pass, or run is
evaluated from that step's output fields and the prior completions, including
the step that just finished.

The run's output is the input of `succeed`. `kith run` prints it.

### Waiting for an outside message

A task or map item that does not `complete` leaves the run at that step. That
is the callback pattern. In Order, Carrier `send to ship` a `Dispatch` and
`goto Shipping`. MockShip (a child, filled on the Carrier prototype) waits
with `send after (rand 1 3) up { kind: "Delivered" }`. Shipping matches
`on up { kind: "Delivered" }` and `complete`s. A host driver MUST deliver
on that same up route. The workflow graph has no `on`; the work item matches.
The join set advances when that item `complete`s.

`World::run_until_idle` leaves those timers pending (Carriers still in
`Shipping`). `World::run` / `kith run` advances the clock until they fire.

### Dynamic graphs

A workflow IR is tagged data with `kind: "workflow"`, `start`, and `steps`.
Uffda expressions MAY build that value. `run plan` MUST accept it. Building
graphs at run time MUST NOT require new source.

### Snapshot

`World::snapshot` MUST emit one Uffda data codec record for every instance
(component machine or workflow run), join sets, mailboxes, ready queue,
clock, RNG, and pending timers. `World::restore` MUST accept that record
plus the program and continue. The record MUST NOT be JSON.

## Syntax tree

- `{ kind: "workflow", name, start, steps: [FlowStep] }`
- `{ name, start: boolean, pattern: Pattern | null, body: StepBody, next: Next | null }`
- `body.kind`: `task` `{ component }`, `map` `{ items, component, reduce, init }`,
  `choice` `{ clauses: [{ pattern, step, input }] }`, `pass` `{ value }`,
  `run` `{ path }`, `succeed`, `fail` `{ error }`
- `next.kind`: `goto` `{ step, input }`, `succeed`, `fail` `{ error }`
- `input` is a message object or null. Null means the step output.

## Example

Inject `{ kind: "Order", orderId: "o-1", items: ["sku-a", "sku-b"], card: "ok" }`
at `Order` MUST accept and charge, then wait on two `Carrier` work items.
After each MockShip `send after (rand 1 3) up { kind: "Delivered" }`, `Summarize`
MUST fold each `Shipped` payload into
`{ kind: "Summarize", orderId: "o-1", total: 2 }`.

Inject `{ kind: "Nope" }` MUST fail the run with `invalid order`.

Inject `{ kind: "Start", items: ["a", "b"] }` at `Import` MUST map two
`Worker`s and reduce them with `Count` to `2`.

## Why this design

- **Step Functions' graph, Kith's syntax.** Task / Map / Choice / Pass /
  Succeed / Fail are the right verbs. JSON state machines are not.
- **Components stay machines; workflows stay flows.** Ping is has-a
  (`send to` / `send up`). Order is a graph (`->` / `pick` / `fanout`).
- **One edge.** A sequential step and a choice arm both name the next step
  with `->`. A second word for that edge is not part of the graph.
- **The name on an edge is a step.** The component is the body of the step
  declared as `name: Pattern`. The object after the name is that step's
  input message, so the graph can pass a new message without hiding the
  node that produced the previous one.
- **The graph is data.** That is what makes dynamic steps, runtime-built
  workflows, and rehydration the same interpreter.
- **Fan-in is an accumulator component, not a collect.** A results array is
  O(n) memory and forces the next step to re-walk every payload. The
  accumulator holds a constant-size aggregate in its fields. Fan-out only
  delivers items; it MUST NOT own that state. Collecting into a list is
  still possible: write an accumulator that concatenates, and accept the
  cost.

## Open questions

- Parallel, Wait-with-timeout, Retry/Catch, JSONPath InputPath / ResultPath
  beyond a single object key. Delayed `send after` is in
  [timers](./timers.spec.md).
- Nested Map iterators that are full subgraphs, not a single kind.
- Exactly-once after a crash versus at-least-once restore.
- Bounded map concurrency so a pending fold queue cannot grow with n.
