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
- A transition MAY send zero or more messages, change fields, complete, and
  change the machine's state. It MUST NOT wait by blocking the dispatcher. The
  first executable slice MUST NOT spawn or destroy children; scene-constructed
  children exist for the parent's lifetime. Dynamic spawn is a
  [workflow](./workflows.spec.md) concern. A component MUST NOT use `fanout`,
  `join`, `goto`, or `step`. A component MAY `complete V` so it can be a
  fan-out work item.

### Routing

- A send MUST name a direction: `send to` a port, `send down` a kind, or
  `send up`. Routing is explicit only: there is no capture phase, and an
  unhandled message MUST NOT travel up unless the send was `send up`.
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

The normative ping program is a [workflow](./workflows.spec.md#declaration)
root with a `Child` component. Components stay local machines:

```kith
component Child {
  start Idle {
    on ({ kind: "Ping", n: number }) {
      complete { kind: "Pong", n: n };
    }
  }
}
```

- `component Name { ... }` declares a kind. Instances of that kind are created
  by a scene.
- `field name: pattern = expression` is local state. The pattern MUST match
  the field's value after every transition.
- `Kind name;` is a port: one child of kind `Kind` bound as `name`.
- `start State { ... }` is the start state. Ordinary states use
  `state Name { ... }`.
- `on (pattern) { ... }` is a transition in that state. The pattern is grouped
  so the handler body `{` is not a Then object. The pattern is a Uffda pattern,
  including object-key captures such as `{ n: number }`. Bindings from the
  match are in scope for the body.
- `export Name;` and `export scene Name` are defined in
  [scenes](./scenes.spec.md#import-and-export).

### Sending

- `send to name V` sends `V` to the named port.
- `send down Kind V` sends `V` to each direct child of that kind.
- `send down any V` sends `V` to every direct child.
- `send up V` sends `V` toward the parent.
- `complete V` marks the instance done for a parent's join set and sends `V`
  up. Completing the root marks it done and does not send up.

A send enqueues work for the dispatcher. The sender MUST NOT observe a
handler's result as a return value. If a later transition needs a result, the
receiver sends a message back.

### Matching

- When an instance takes a message, the runtime matches it against the `on`
  patterns of the current state, in declaration order, and takes the first
  match.
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
  long as the parent.
- The first slice MUST NOT spawn extra children of a constructed kind. Fan-out
  is [workflows](./workflows.spec.md).

## Syntax tree

The grammar MUST project source to this tagged data shape. The interpreter MUST
accept the same shape constructed without parsing. Uffda pattern and expression
subtrees keep Uffda's own AST kinds (`over`, `variable`, `invocation`,
`number`, `resolve`, …).

- `{ kind: "program", imports: [Import], exports: [Export], components: [Component], workflows: [Workflow], scenes: [Scene] }`
- `{ kind: "import", moduleUrl: string, names: [string] }`
- `{ kind: "export", name: string }`
- `{ kind: "component", name: string, fields: [Field], ports: [Port], states: [State] }`
- `{ kind: "field", name: string, pattern: Pattern, init: Expression }`
- `{ kind: "port", component: string, name: string }`
- `{ kind: "scene", name: string, root: Instance }`
- `{ kind: "instance", component: string, key: string | null, children: [Instance] }`
- `{ kind: "state", name: string, start: boolean, handlers: [Handler] }`
- `{ kind: "handler", pattern: Pattern, body: [Statement] }`
- `{ kind: "assign", field: string, expression: Expression }`
- `{ kind: "sendUp", message: Expression }`
- `{ kind: "sendTo", port: string, message: Expression }`
- `{ kind: "sendDown", component: string | null, message: Expression }`
- `{ kind: "complete", message: Expression }`

Exactly one state in a component MUST have `start: true`. Field `pattern` and
`init` MAY be omitted when the field is untyped or uninitialized; a missing
`init` is `undefined` until a transition assigns it. The root instance's `key`
MUST be `null`. An unnamed child's `key` MUST be `null`. `sendDown.component`
MUST be `null` when the send is `send down any`.

## Example

Injecting `{ kind: "Ping", n: 7 }` at the `Ping` workflow root of scene `Main`
MUST `send to child`, the child MUST `complete` `{ kind: "Pong", n: 7 }`, and
`Ping.heard` MUST become `7`. See [workflows](./workflows.spec.md#example).

## Why this design

- **Has-a rather than is-a.** Inheritance would let a child override a parent's
  transitions and reach into its state. That hides the message protocol and
  breaks the tree as the only composition. Ports plus a scene keep the protocol
  explicit.
- **No function calls for behavior.** A call stack is a hidden scheduling and
  error path. Messages make routing, failure, and parallelism visible to the
  dispatcher.
- **Pattern-matched receives.** Uffda already matches values. A separate switch
  language would duplicate it.
- **`send to` / `send down` / `send up`.** A named port is not a kind. `to`
  names the port; `down` names the kind; `up` is the ancestor chain.

## Open questions

- **Entity-like parts.** Whether a component MAY have unnamed owned parts that
  are not independently dispatched actors.
- **How higher languages specialize a component** (restriction of messages,
  durable identity, a single mailbox as a consistency boundary) without
  introducing is-a. Deferred until a language such as Coleslaw maps onto Kith.
- **Goto state.** Components still have no `goto`. Workflows use `goto` to
  enter another step; see [workflows](./workflows.spec.md).
- **`send up P V`.** Filtering ancestors with a pattern is not in this slice.
