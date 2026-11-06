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
- A transition MAY send zero or more messages, change fields, change the
  machine's state, spawn children, or spawn transition work. It MUST NOT wait
  by blocking the dispatcher; waiting is joining or receiving a later message.

### Routing

- A send MUST name a direction: bubble, or down to a named child, or down to
  every child.
- Bubble offers the message to the parent, then to that parent's parent, until
  a transition consumes it or the root does not.
- Down offers the message to the named child, or to each child. It MUST NOT
  skip a generation.
- A component MUST NOT send a message by calling another instance as a
  function.

## Mechanism

### Declaration

A component declaration names the kind, optional fields, optional children, and
the machine. The following sketch establishes the shape; token spelling is not
frozen (see [overview](./overview.spec.md#open-questions)).

```text
component Counter {
  field count: (int) = 0;
  child Display display;

  start Idle {
    on (Increment n) {
      count -> (add count n);
      down display (Show count);
    }
    on (Query) {
      bubble (Count count);
    }
  }
}
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
  pattern. Bindings from the match are in scope for the body.

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
- An unmatched message that was bubbling MUST continue to the next ancestor.
- An unmatched message that was sent down to this instance MUST NOT be
  forwarded to this instance's children unless a transition sends it down.

### Identity and lifetime

- Each instance MUST have an identity unique among instances of its kind in the
  running program.
- Creating a child is a transition's effect. Destroying a child is a
  transition's effect. The dispatcher MUST NOT create or destroy instances
  except as those effects.

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

- **Unmatched down messages.** Whether they are an error, a diagnostic, or
  silently dropped.
- **Whether bubble is opt-in per message** or the default for unhandled
  messages that originated down the tree (DOM-style).
- **Capture.** Whether there is a downward capture phase before the target
  handles the message, or only explicit down and bubble.
- **Dynamic children.** Whether children are only those declared, or a
  transition MAY spawn extra children of a declared kind (fan-out lives
  primarily on [workflows](./workflows.spec.md)).
- **Entity-like parts.** Whether a component MAY have unnamed owned parts that
  are not independently dispatched actors.
- **How higher languages specialize a component** (restriction of messages,
  durable identity, a single mailbox as a consistency boundary) without
  introducing is-a. Deferred until a language such as Coleslaw maps onto Kith.
