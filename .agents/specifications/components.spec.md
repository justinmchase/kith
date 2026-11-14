# Components

This chapter defines components: kinds with fields, typed ports, and state
machines, and sending messages to a port, down by kind, or up the tree a
[scene](./scenes.spec.md) built. Dispatch of those messages is defined in
[dispatch](./dispatch.spec.md). Terms are defined in the
[glossary](./glossary.spec.md).

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be interpreted
as described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Definitions

- **Component declaration**: the source that names a component, its fields, its
  ports, and its state machine.
- **Field**: named state a component instance holds, matched by a pattern.
- **Port**: a typed named slot for one child instance, filled by a scene.

## Axioms

- A component is a kind. It is not a kind of another component.
- An instance other than the root has exactly one parent.
- An instance's machine state changes only by completing a transition.
- The only communication that causes behavior is a message.
- A transition is enabled only by matching a message against a pattern in the
  current state.

## Constraints

### Composition

- A component MUST NOT inherit, override, or subtype another component.
- Reuse MUST be by composition in a [scene](./scenes.spec.md), wrapping an
  instance, or sending it messages.
- Nothing outside an instance MAY hold a child of that instance. Children are
  reached only through their parent: `send to` a port, or `send down` a kind.

### Behavior

- A component MUST declare exactly one start state.
- Choosing what happens MUST be done by a transition. Expressions MUST NOT send
  messages or change machine state.
- A transition MUST be deterministic: the same machine state and the same
  matched message MUST produce the same field changes and the same sends.
- A transition MAY send zero or more messages, change fields, `print`,
  complete, and change the machine's state. It MUST NOT wait by blocking the
  dispatcher. The
  first executable slice MUST NOT spawn or destroy children; scene-constructed
  children exist for the parent's lifetime. Dynamic spawn is a
  [workflow](./workflows.spec.md) concern. A component MUST NOT use `fanout`,
  `then`, `pick`, `run`, `succeed`, or `fail`. A component MAY `complete V` so
  it can be a task or map work item.

### Routing

- A send MUST name a direction: `send to` a port, `send down` a kind,
  `send up`, or `send self`. Routing is explicit only: there is no capture
  phase, and an unhandled message MUST NOT travel up unless the send was
  `send up`. A send with no route is invalid; self is not implied.
- `send up` offers the message to the parent, then to that parent's parent,
  until a transition consumes it or the root does not. An unmatched send up at
  the root MUST be dropped with no error.
- `send to name V` offers `V` to the child that fills port `name`. An unknown
  port MUST fail that send with a diagnostic.
- `send down Kind V` offers `V` to each direct child of kind `Kind`, including
  a named port whose kind is `Kind`. It MUST NOT skip a generation. Zero
  matching children MUST fail that send with a diagnostic.
- `send down any V` offers `V` to every direct child. Zero children MUST fail.
- An unmatched handler at a selected child MUST fail that send with a
  diagnostic; it MUST NOT be forwarded to the child's children.
- Several matching children MUST each receive one send, independently.
- A component MUST NOT send a message by calling another instance as a
  function.

## Mechanism

### Declaration

A component declaration names the kind, optional fields, optional ports, and
the machine. A scene constructs instances and fills ports.

The normative ping program is a parent/child machine. `Ping` owns `Child` by
port; they talk with `send to` and `send up`. Workflows are a different
primitive ([workflows](./workflows.spec.md)).

```kith
component Child {
  start {
    on down "Ping" n:number {
      send up "Pong" n;
    }
  }
}

component Ping {
  field heard: number = 0;
  Child child;

  start {
    on down "Ping" n:number {
      send to child "Ping" n;
    }
    on up "Pong" n:number {
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

- `component Name { ... }` declares a kind. Instances of that kind are created
  by a scene.
- `field name: pattern = expression` is local state. Every write MUST
  match `pattern` as a Uffda scalar value: the init, `name -> E`, and
  `into` field arguments. A miss MUST fail that write with a diagnostic.
  `= expression` MAY be omitted; a missing init is **null**. `number?`
  matches null or a number and MUST NOT match a string: Uffda Maybe that
  skips a present non-null value is not a match for a stored field.
  The value used is the value that matched.
- `Kind name;` is a port: one child of kind `Kind` bound as `name`.
- `start { ... }` is the start state with implicit name `Start`. That name
  is not a `goto` target. Use unnamed start when the machine never returns
  to it. `start Name { ... }` names the start; `goto Name` returns there.
  Ordinary states use `state Name { ... }`.
- `on pattern { ... }` is a transition in that state. The pattern is a
  Uffda Then of prefixes up to the handler `{`: a quoted string, a
  capture (`n:number`, `v:{ kind: "Done" status body }`), a single atom
  (`any`, a type name), or one object. Several prefixes match a sequence
  (`on self "Check" a:200 b:200 c:200`). One prefix is that pattern, not
  wrapped. `on (pattern) { ... }` groups a full Uffda pattern (Or / Then /
  Pipe) so `|` and juxtaposition cannot eat the handler body. Bindings from
  the match are in scope for the body. Matching is Uffda pattern matching
  ([matching](#matching)).
- `on` MAY list routes joined by `or`: `up`, `down`, and `self`.
  `on down pattern` matches a send to or down, and an inject. `on up`
  matches a child send up. `on self` matches `send self`.
  `on up or down or self pattern` matches any of those. A name MUST NOT
  appear twice. `on pattern` with no list is the same as
  `on up or down or self pattern`. `on init { ... }` is not in that list:
  no pattern, construction only. It MUST NOT be combined with `or`. A
  parent sending a child's reply shape is unmatched down; a child sending
  a parent's request shape continues up.
- `on item: any { ... }` is a capture: the whole message is bound as `item`.
  A map accumulator uses that for each completion; see
  [reduce](./workflows.spec.md#declaration). `on "Ping" n:number` is Then
  of a tag and a payload: it matches the sequence `["Ping", n]`.
- `else { ... }` is the default transition. It MUST be last if present. It
  is the same handler as `on any`: it matches any remaining message. `else`
  MAY be a single statement: `else complete { kind: "Summarize" total };`.
  `on any` remains valid; `else` is the spelling for a fallthrough clause.
- `goto Name;` enters another **named** state of this component. The next
  message is matched in `Name`. `Name` MUST be a `start Name` or
  `state Name` of this component. Unnamed `start { }` is not a target:
  `goto Start` MUST fail check unless the start was declared `start Start`.
  An unknown name MUST fail check. `goto` is a terminal: see
  [terminal statements](#terminal-statements). Workflows change step with
  `then` / `pick`, not `goto`.
- `export Name;` and `export scene Name` are defined in
  [scenes](./scenes.spec.md#import-and-export).

### Sending

- `send to name V…` sends the message to the named port.
- `send down Kind V…` sends the message to each direct child of that kind.
- `send down any V…` sends the message to every direct child.
- `send up V…` sends the message toward the parent.
- `send self V…` sends the message to this instance. It is taken on a later
  turn and matches a handler whose route list includes `self` (`on self`,
  `on up or self`, unqualified `on`). It MUST NOT match `on down` alone.
  `send V` with no route is invalid.
- One value after the route is that value. Two or more values are the
  array of those values, in order: `send self "Check" s1 s2 s3` sends
  `["Check", s1, s2, s3]`. `send self "Tick"` sends `"Tick"`. An object
  is one value: `send up { kind: "Done" status body }` still sends an
  object. `complete` and `print` take the same value list.
- `send after Delay Route V…` is the same send, delivered later. `Delay` is
  seconds. See [timers](./timers.spec.md). Self is written
  `send after 5 self "Tick"`. Order's `MockShip` uses
  `send after (rand 1 3) up { kind: "Delivered" }` so Carrier's `Shipping`
  state is the callback wait. A host driver for a child kind MUST deliver
  on that same up route, not by poking the parent.
- I/O (an HTTP request, a queue, a device) is the same callback: the turn
  MUST NOT await a response. A child receives a request down; a later
  `send up` carries the result. `(fetch url)` MUST NOT be an expression.
  See [runtime](./runtime.spec.md#host-io).
- `complete V` finishes this instance as a work item. It MUST send `V` up
  (the same route as `send up`) unless this instance is the root, and MUST
  mark the instance complete. Completing the root marks it done and does
  not send up. `complete` is a terminal: see
  [terminal statements](#terminal-statements).
- `complete` MUST NOT change machine state. The instance stays in the state
  that ran `complete` (Shipping, Start, …). There is no `Completed` state.
  Complete is a flag, not a `goto`.
- A complete instance MUST NOT run another transition. A later message
  follows unmatched routing: down or inject fails; up continues toward the
  root. Scene-constructed children that outlive one request (`Http`,
  `MockShip`, Ping's `Child`) MUST `send up`, not `complete`, so they can
  take another message.
- A message object (`send`, `complete`, `print`) MAY omit commas between
  entries. An identifier with no `:` is `name: name`.
  `{ kind: "Done" status body }` is `{ kind: "Done", status: status, body: body }`.
  Uffda expressions that are not these message objects keep Uffda's object
  commas. An object pattern in `on` projects a Uffda Over and MAY omit
  commas the same way: `{ kind: "GET" url: any }`. A bare identifier in an
  object pattern binds that field as `any`. Field values are one prefix, so
  `{ kind: "GET" url: any }` does not need grouping. A grouped
  `(number & not 200)` is still a full Uffda pattern.
- `print E` evaluates `E` and records the value as a diagnostic. It MUST NOT
  be an expression. `kith run` writes those records to stderr as data codec
  lines; stdout remains the run result. Uffda globals such as `length` MAY
  appear in expressions because they are pure.

A send enqueues work for the dispatcher. The sender MUST NOT observe a
handler's result as a return value. If a later transition needs a result, the
receiver sends a message back.

### Terminal statements

A handler body is a synchronous prefix followed by at most one terminal.

- Assign, `print`, and `send` run on this turn, in order, before any
  terminal. A send only enqueues: it MUST NOT run the receiver on this
  turn. `send after` only arms a timer.
- The terminal, if present, MUST be the last statement. It is either
  `goto Name` or `complete V`. A handler MUST NOT contain both. A handler
  MUST NOT contain more than one `goto` or more than one `complete`.
  Check MUST reject a terminal that is not last.
- `goto` after `send` is the required order when both appear. It is safe:
  the later turn matches in the state that `goto` entered. `goto` then
  `send` is invalid because the terminal would not be last.
- `goto` and `complete` MUST end the transition: statements after them
  MUST NOT run.

### Matching

- When an instance takes a message, the runtime matches it against the `on`
  handlers of the current state, in declaration order, and takes the first
  match whose route list accepts the message and whose pattern matches.
  Matching is Uffda value pattern matching (`uffda-runtime` Pattern). A Then
  of several prefixes matches an array of that many values as an iterable
  input; every other pattern matches the message as one scalar value. The
  array MUST have exactly as many elements as prefixes. `else` is matched
  last, as `any` with the unqualified route list. It MUST NOT match init.
- A listed name matches as follows: `down` is send to, send down, or
  inject; `up` is send up; `self` is send self. The handler matches if any
  listed name matches. `on init` MUST match only construction init.
  Inject matches `down`, not `self`.
- A matching transition consumes the message: send up MUST NOT continue.
  Several children selected by one `send down` are independent sends.
- An unmatched message that was travelling up MUST continue to the next
  ancestor, and MUST be dropped with no error if the root does not consume it.
- An unmatched message that was sent to or down to this instance MUST fail that
  send with a diagnostic. It MUST NOT be forwarded to this instance's children.

### Identity and lifetime

- Each instance MUST have an identity unique among instances of its kind in the
  running program.
- Scene-constructed children MUST be created when the parent instance is
  created, one instance per construction, in declaration order. They live as
  long as the parent. Completing a child does not destroy it; it only marks
  it done so it will not run another transition.
- After the scene tree is constructed, the runtime MUST offer each
  component instance init in its start state, before any injected message.
  Init has no payload. Only `on init` matches it. Unmatched init MUST be
  dropped with no error, including at the root. Workflows and later
  fan-out work items MUST NOT be offered init.
- The first slice MUST NOT spawn extra children of a constructed kind. Fan-out
  is [workflows](./workflows.spec.md).

## Syntax tree

The grammar MUST project source to this tagged data shape. The interpreter MUST
accept the same shape constructed without parsing. Serialized trees, program
artifacts, and workflow snapshots MUST use the Uffda data codec, not JSON.
Uffda pattern and expression subtrees keep Uffda's own AST kinds (`over`,
`variable`, `invocation`, `number`, `resolve`, …).

- `{ kind: "program", imports: [Import], exports: [Export], components: [Component], workflows: [Workflow], scenes: [Scene] }`
- `{ kind: "import", moduleUrl: string, names: [string] }`
- `{ kind: "export", name: string }`
- `{ kind: "component", name: string, fields: [Field], ports: [Port], states: [State] }`
- `{ kind: "field", name: string, pattern: Pattern, init: Expression }`
- `{ kind: "port", component: string, name: string }`
- `{ kind: "scene", name: string, root: Instance }`
- `{ kind: "instance", component: string, key: string | null, children: [Instance] }`
- `{ kind: "state", name: string, start: boolean, named: boolean, handlers: [Handler] }`
- `{ kind: "handler", pattern: Pattern, item: Pattern | null, route: ["up" | "down" | "self" | "init"], body: [Statement] }`
- `{ kind: "assign", field: string, expression: Expression }`
- `{ kind: "sendUp", message: Expression | Sequence, after: Expression | null }`
- `{ kind: "sendTo", port: string, message: Expression | Sequence, after: Expression | null }`
- `{ kind: "sendDown", component: string | null, message: Expression | Sequence, after: Expression | null }`
- `{ kind: "send", message: Expression | Sequence, after: Expression | null }`
- `{ kind: "complete", message: Expression | Sequence }`
- `{ kind: "print", expression: Expression | Sequence }`
- `{ kind: "sequence", items: [Expression] }`
- `{ kind: "goto", state: string }`

Exactly one state in a component MUST have `start: true`. Unnamed `start { }`
MUST project `name: "Start"` and `named: false`. `start Name` and
`state Name` MUST project `named: true`. Field `pattern` and
`init` MAY be omitted when the field is untyped or uninitialized; a missing
`init` is `undefined` until a transition assigns it. The root instance's `key`
MUST be `null`. An unnamed child's `key` MUST be `null`. `sendDown.component`
MUST be `null` when the send is `send down any`. `handler.item` MAY be omitted
when the transition matches one message. `handler.route` is a list. `on down`
MUST project `["down"]`. `on up or self` MUST project `["up", "self"]`.
Unqualified `on` and `else` MUST project `["up", "down", "self"]`. `on init`
MUST project `["init"]` and pattern `any`. An omitted `route` on a
constructed tree is the unqualified list. A send, complete, or print of
two or more values MUST project `{ kind: "sequence", items: [...] }`. One
value MUST project that expression, not a sequence of length one. A
handler Then of several prefixes MUST project `{ kind: "then", patterns }`.
One prefix MUST project that pattern, not a Then of length one.
`handler.item` MAY still be set on a constructed tree; source uses Then.

## Example

Injecting `["Ping", 7]` at `Ping` MUST `send to child`, Child MUST
`send up` `["Pong", 7]`, and `Ping.heard` MUST be `7`. The Order
workflow is in [workflows](./workflows.spec.md#example).

## Why this design

- **Has-a rather than is-a.** Inheritance would let a child override a parent's
  transitions and reach into its state. That hides the message protocol and
  breaks the tree as the only composition. Ports plus a scene keep the protocol
  explicit.
- **No function calls for behavior.** A call stack is a hidden scheduling and
  error path. Messages make routing, failure, and parallelism visible to the
  dispatcher.
- **Pattern-matched receives.** Uffda already matches values: a Then of
  prefixes is an iterable match, an Over is an object match. A separate
  switch language would duplicate it. Kith only stops Then before the
  handler `{` (a Uffda Then would take `{` as another object) and allows
  comma-optional Over entries that still project Uffda Over.
- **`send to` / `send down` / `send up` / `send self`.** A named port is not
  a kind. `to` names the port; `down` names the kind; `up` is the ancestor
  chain; `self` is this instance on a later turn. A missing route would look
  like a parent sending down.
- **One terminal, last.** Several `goto`s would leave the next state's
  meaning ambiguous. Running statements after `goto` or `complete` would
  hide that the turn already ended. Sends stay later-turn, so `send` then
  `goto` is the only order that needs to exist.

## Open questions

- **Entity-like parts.** Whether a component MAY have unnamed owned parts that
  are not independently dispatched actors.
- **How higher languages specialize a component** (restriction of messages,
  durable identity, a single mailbox as a consistency boundary) without
  introducing is-a. Deferred until a language such as Coleslaw maps onto Kith.
- **`send up P V`.** Filtering ancestors with a pattern is not in this slice.
