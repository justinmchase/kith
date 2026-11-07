# Components

This chapter defines components: has-a composition, children, state machines,
and sending messages up or down the tree. Dispatch of those messages is defined
in [dispatch](./dispatch.spec.md). Terms are defined in the
[glossary](./glossary.spec.md).

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be interpreted
as described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Definitions

- **Component declaration**: the source that names a component, its fields, its
  children, and its state machine.
- **Child declaration**: a named slot on a component for an owned instance of a
  declared component or workflow.
- **Field**: named state a component instance holds, matched by a pattern.

## Axioms

- A component has-a its children. It is not a kind of another component.
- An instance other than the root has exactly one parent.
- An instance's machine state changes only by completing a transition.
- The only communication that causes behavior is a message.
- A transition is enabled only by matching a message against a pattern in the
  current state.

## Constraints

### Composition

- A component MUST NOT inherit, override, or subtype another component.
- Reuse MUST be by composition: declaring a child, wrapping an instance, or
  sending it messages.
- Nothing outside an instance MAY hold a child of that instance. Children are
  reached only through their parent, by name, or as the target of a down send
  from that parent.

### Behavior

- A component MUST declare exactly one start state.
- Choosing what happens MUST be done by a transition. Expressions MUST NOT send
  messages or change machine state.
- A transition MUST be deterministic: the same machine state and the same
  matched message MUST produce the same field changes and the same sends.
- A transition MAY send zero or more messages, change fields, and change the
  machine's state. It MUST NOT wait by blocking the dispatcher. The first
  executable slice MUST NOT spawn or destroy children; declared children exist
  for the parent's lifetime. Dynamic spawn is a [workflow](./workflows.spec.md)
  concern.

### Routing

- A send MUST name a direction: bubble, or down to a named child, or down to
  every child. Routing is explicit only: there is no capture phase, and an
  unhandled message MUST NOT bubble unless the send was `bubble`.
- Bubble offers the message to the parent, then to that parent's parent, until
  a transition consumes it or the root does not. An unmatched bubble at the
  root MUST be dropped with no error.
- Down offers the message to the named child, or to each child. It MUST NOT
  skip a generation. An unmatched down MUST fail that send with a diagnostic;
  it MUST NOT be forwarded to the child's children.
- A component MUST NOT send a message by calling another instance as a
  function.

## Mechanism

### Declaration

A component declaration names the kind, optional fields, optional children, and
the machine. This form is the declaration syntax of the first slice.

```text
component Child {
  start Idle {
    on { kind: "Ping", n: number } {
      bubble { kind: "Pong", n: n };
    }
  }
}

component Parent {
  field heard: (number) = 0;
  child Child child;

  start Idle {
    on { kind: "Ping", n: number } {
      down child { kind: "Ping", n: n };
    }
    on { kind: "Pong", n: number } {
      heard -> n;
    }
  }
}

root Parent;
```

- `component Name { ... }` declares a kind. Instances of that kind are created
  as children or as the program root.
- `child Kind name` is has-a: this instance owns a child of kind `Kind` bound
  to `name`.
- `field name: pattern = expression` is local state. The pattern MUST match
  the field's value after every transition.
- `start State { ... }` is the start state. Ordinary states use
  `state Name { ... }`.
- `on pattern { ... }` is a transition in that state. The pattern is a Uffda
  pattern, including object-key captures such as `{ n: number }`. Bindings from the match are
  in scope for the body.
- `root Name;` names the kind of the program root. A program MUST declare
  exactly one root, and that name MUST be a component in the same module.

### Sending

- `bubble message` sends toward the parent.
- `down name message` sends to the named child.
- `down message` sends to every child.

A send enqueues work for the dispatcher. The sender MUST NOT observe a
handler's result as a return value. If a later transition needs a result, the
receiver sends a message back.

### Matching

- When an instance takes a message, the runtime matches it against the `on`
  patterns of the current state, in declaration order, and takes the first
  match.
- A matching transition consumes the message: bubble MUST NOT continue, and
  down MUST NOT be offered to further children of the same send, except that a
  down to every child is one send per child, each matched independently.
- An unmatched message that was bubbling MUST continue to the next ancestor,
  and MUST be dropped with no error if the root does not consume it.
- An unmatched message that was sent down to this instance MUST fail that send
  with a diagnostic. It MUST NOT be forwarded to this instance's children.

### Identity and lifetime

- Each instance MUST have an identity unique among instances of its kind in the
  running program.
- Declared children MUST be created when the parent instance is created, one
  instance per slot, in declaration order. They live as long as the parent.
- The first slice MUST NOT spawn extra children of a declared kind. Fan-out is
  [workflows](./workflows.spec.md).

## Syntax tree

The grammar MUST project source to this tagged data shape. The interpreter MUST
accept the same shape constructed without parsing. Uffda pattern and expression
subtrees keep Uffda's own AST kinds (`over`, `variable`, `invocation`,
`number`, …).

- `{ kind: "program", components: [Component], root: string }`
- `{ kind: "component", name: string, fields: [Field], children: [Child], states: [State] }`
- `{ kind: "field", name: string, pattern: Pattern, init: Expression }`
- `{ kind: "child", component: string, name: string }`
- `{ kind: "state", name: string, start: boolean, handlers: [Handler] }`
- `{ kind: "handler", pattern: Pattern, body: [Statement] }`
- `{ kind: "assign", field: string, expression: Expression }`
- `{ kind: "bubble", message: Expression }`
- `{ kind: "down", child: string, message: Expression }`
- `{ kind: "downAll", message: Expression }`

Exactly one state in a component MUST have `start: true`. Field `pattern` and
`init` MAY be omitted when the field is untyped or uninitialized; a missing
`init` is `undefined` until a transition assigns it.

## Example

Injecting `{ kind: "Ping", n: 7 }` at the `Parent` root MUST `down` the child,
the child MUST `bubble` `{ kind: "Pong", n: 7 }`, and `Parent.heard` MUST
become `7`.

## Why this design

- **Has-a rather than is-a.** Inheritance would let a child override a parent's
  transitions and reach into its state. That hides the message protocol and
  breaks the tree as the only composition. Wrapping and children keep the
  protocol explicit.
- **No function calls for behavior.** A call stack is a hidden scheduling and
  error path. Messages make routing, failure, and parallelism visible to the
  dispatcher.
- **Pattern-matched receives.** Uffda already matches values. A separate switch
  language would duplicate it and drift from the patterns higher languages use
  as shapes.

## Open questions

- **Entity-like parts.** Whether a component MAY have unnamed owned parts that
  are not independently dispatched actors.
- **How higher languages specialize a component** (restriction of messages,
  durable identity, a single mailbox as a consistency boundary) without
  introducing is-a. Deferred until a language such as Coleslaw maps onto Kith.
- **Goto state.** Whether a transition MAY name the next state, or the machine
  stays in the start state until a later slice. The first slice keeps the
  current state unless a statement sets it; no `goto` syntax yet.
