# Components

This chapter defines components: kinds with fields, typed ports, and state
machines, and sending messages to a port, down to children, or up the tree a
[scene](./scenes.spec.md) built. It also defines how long an instance lives
and which fields a state holds. Dispatch of those messages is defined in
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
  A field declared on the component is in scope in every state. A field may
  instead name the states it belongs to
  ([state-scoped fields](#state-scoped-fields)). It is still one field.
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
  reached only through their parent: `send to` a port, or `send down` (a kind,
  `any`, or a filter). The parent does not receive its children as a value.

### Behavior

- A component MUST declare exactly one start state.
- Choosing what happens MUST be done by a transition. Expressions MUST NOT send
  messages, change machine state, or construct or release children.
- A transition MUST be deterministic: the same machine state and the same
  matched message MUST produce the same field changes, the same sends, the
  same children constructed or released, and the same entry-pattern and
  post-condition matches.
- A transition MAY send zero or more messages, change fields, evaluate
  expression statements, construct or release children, complete, and change
  the machine's state. It MUST NOT wait by blocking the dispatcher.
  Scene-constructed children exist for the parent's lifetime. A transition
  MAY construct more children. A child the parent no longer holds MUST be
  destroyed. A component MAY `complete V`.

### Routing

- A send MUST name a direction: `send to` a port, `send down` (a kind,
  `any`, or a filter), `send up`, or `send self`. Routing is explicit only:
  there is no capture
  phase, and an unhandled message MUST NOT travel up unless the send was
  `send up`. A send with no route is invalid; self is not implied.
- `send up` offers the message to the parent, then to that parent's parent,
  until a transition consumes it or the root does not. An unnamed child's
  `send up`, whether the scene constructed it or a transition did, is this
  same walk. The parent matches it with `on up`. There is one `send up`
  route. An unmatched send up at the root MUST be dropped with no error.
- `send to name V` offers `V` to the child that fills port `name`. An unknown
  port MUST fail that send with a diagnostic. An unnamed child has no port
  name, so `send to` cannot address it, and the child MUST NOT gain a port
  name. `send to` a port does not deliver to an unnamed sibling.
- `send down Kind V` offers `V` to each direct child of kind `Kind`. One such
  send selects a port child and an unnamed child of that same kind together.
  The unnamed child may be scene-constructed or dynamic. It MUST NOT skip a
  generation. Zero matching children MUST fail that send with a diagnostic.
  `send down Other` does not deliver to a child whose kind is not `Other`.
- `send down any V` offers `V` to every direct child. Zero children MUST fail.
  Kind equality and `any` keep these failure rules. Neither is a filter.
- A filtered `send down` is a third form. Finding a child is this send. The
  parent does not receive the children as a value. Its source spelling is
  open ([open questions](#open-questions)). The filter matches a Uffda
  pattern against the record the parent already holds for each direct child,
  and against nothing else. That record is
  `{ kind, port, key }`. `kind` is the component kind. `port` is the port
  name, or null when the child is unnamed. `key` is the glossary
  [key](./glossary.spec.md#components-and-trees). A scene-constructed child's
  `key` is null, including a port child: the scene instance node's `key`
  field holds the port name, and that name is this record's `port`, not a
  non-null key. A dynamic child's `port` is null, and its `key` is the value
  the transition supplied, or null. The record does not include the child's
  fields, machine state, or mailbox. Each match gets its own send, and the
  child matches that send with `on down`. Zero matches MUST fail the send
  with a diagnostic.
- An unmatched handler at a selected child MUST fail that send with a
  diagnostic; it MUST NOT be forwarded to the child's children. The same
  rule applies to a child a filter selected.
- Several matching children MUST each receive one send, independently.
- A component MUST NOT send a message by calling another instance as a
  function.

## Mechanism

### Declaration

A component declaration names the kind, optional fields, optional ports, and
the machine. A scene constructs instances and fills ports.

The normative ping program is a parent/child machine. `Ping` owns `Child` by
port; they talk with `send to` and `send up`.

```kith
component Child {
  start {
    on down @Ping n:number {
      send up @Pong n;
    }
  }
}

component Ping {
  field heard: number = 0;
  child: Child;

  start {
    on down @Ping n:number {
      send to child @Ping n;
    }
    on up @Pong n:number {
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
  by a scene, or by a parent's transition
  ([dynamic children](#dynamic-children)). The instance is the same either way.
- `field name: pattern = expression` is local state. Every write MUST
  match `pattern` as a Uffda scalar value: the init and `name -> E`.
  A miss MUST fail that write with a diagnostic.
  `= expression` MAY be omitted; a missing init is **null**. `number?`
  matches null or a number and MUST NOT match a string: Uffda Maybe that
  skips a present non-null value is not a match for a stored field.
  The value used is the value that matched. This field is in scope in every
  state. A field that instead names the states it belongs to is specified
  in [state-scoped fields](#state-scoped-fields). That spelling is open.
- `name: Kind;` is a port: one child of kind `Kind` bound as `name`.
  `Kind` is a component.
- `start { ... }` is the start state with implicit name `Start`. That name
  is not a `goto` target. Use unnamed start when the machine never returns
  to it. `start Name { ... }` names the start; `goto Name` returns there.
  Ordinary states use `state Name { ... }`.
- `on pattern { ... }` is a transition in that state. The pattern is a
  Uffda Then of prefixes up to the handler `{`: a quoted string, a
  capture (`n:number`, `v:{ kind: "Done" status body }`), a single atom
  (`any`, a type name), or one object. A non-empty `{ ... }` after another
  prefix is that object. `on "Tick" {}` is an empty body, not an empty
  object pattern. Several prefixes match a sequence
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
  `on @Ping n:number` is Then of the symbol `Ping` and a payload. It matches
  that symbol and `n`, and MUST NOT match the string `"Ping"`.
- `else { ... }` is the default transition. It MUST be last if present. It
  is the same handler as `on any`: it matches any remaining message. `else`
  MAY be a single statement: `else complete { kind: "Summarize" total };`.
  `on any` remains valid; `else` is the spelling for a fallthrough clause.
- `goto Name;` enters another **named** state of this component. The next
  message is matched in `Name`. `Name` MUST be a `start Name` or
  `state Name` of this component. Unnamed `start { }` is not a target:
  `goto Start` MUST fail check unless the start was declared `start Start`.
  An unknown name MUST fail check. `goto` is a terminal: see
  [terminal statements](#terminal-statements).
- `export Name;` and `export scene Name` are defined in
  [scenes](./scenes.spec.md#import-and-export).

The forms above stay. The following forms sit beside them. Their source
spellings are open ([open questions](#open-questions)). They MUST NOT change
`field`, `on`, `send down Kind`, or `send down any`.

- A filtered `send down`, specified in [routing](#routing).
- A statement that constructs a direct child, and a statement that releases
  a dynamic child, specified in [dynamic children](#dynamic-children) and
  [identity and lifetime](#identity-and-lifetime).
- An entry pattern and a post-condition on a state, specified in
  [state conditions](#state-conditions).
- A field that names the states it belongs to, specified in
  [state-scoped fields](#state-scoped-fields).

### Sending

- `send to name V…` sends the message to the named port. An unnamed child has
  no port name, so `send to` cannot address it.
- `send down Kind V…` sends the message to each direct child of that kind, a
  port child and an unnamed child of that kind together.
- `send down any V…` sends the message to every direct child.
- A filtered `send down` sends the message to each direct child whose record
  matches, one send each. The parent does not receive those children as a
  value. `send down Kind` and `send down any` are not this form.
- `send up V…` sends the message toward the parent. An unnamed child uses
  this same send. The parent matches it with `on up`.
- `send self V…` sends the message to this instance. It is taken on a later
  turn and matches a handler whose route list includes `self` (`on self`,
  `on up or self`, unqualified `on`). It MUST NOT match `on down` alone.
  `send V` with no route is invalid.
- One value after the route is that value. Two or more values are the
  array of those values, in order: `send self "Check" s1 s2 s3` sends
  `["Check", s1, s2, s3]`. `send self "Tick"` sends `"Tick"`. An object
  is one value: `send up { kind: "Done" status body }` still sends an
  object. `complete` takes the same value list.
- A newline between those values does not end the statement.
  `send to name @Listen` with the host and port arguments on following
  lines is one send of three values: the symbol `Listen`, the host, and
  the port.
- `@Name` is the global registry symbol `Name`. It is the same value as
  `s"Name"`, `@symbol"Name"`, and `(symbol "Name")`. It is not a keyword
  escape. The string `"Name"` is a different value. `on @Name` matches
  that symbol and MUST NOT match the string.
- `send after Delay Route V…` is the same send, delivered later. `Delay` is
  seconds. See [timers](./timers.spec.md). Self is written
  `send after 5 self "Tick"`. Order's `MockShip` uses
  `send after (rand 1 3) up { kind: "Delivered" }` so Carrier's `Shipping`
  state is the callback wait. A host driver for a child kind MUST deliver
  on that same up route, not by poking the parent.
- I/O that takes time is a later message: the turn MUST NOT await a
  response. `send Route (http_get url)` starts the work this turn and
  delivers the result on that route when the host is done. Several async
  arguments in **one** send wait for all and deliver one message, in
  argument order (like `Promise.all`). Two `send` statements remain two
  messages. `goto` after that send is this turn; `on` in the entered state
  is the switch. A child capability (MockShip, a scene-filled `Http`) MAY
  still take a request down and `send up` later. See
  [runtime](./runtime.spec.md#host-io).
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
- A message object (`send`, `complete`) MAY omit commas between entries.
  An identifier with no `:` is `name: name`.
  `{ kind: "Done" status body }` is `{ kind: "Done", status: status, body: body }`.
  A key MAY be a string, so a name that is not an identifier can be written
  `{ "Content-Type": "text/plain" }`.
  Uffda expressions that are not these message objects keep Uffda's object
  commas. An object pattern in `on` projects a Uffda Over and MAY omit
  commas the same way: `{ kind: "GET" url: any }`. A bare identifier in an
  object pattern binds that field as `any`. Field values are one prefix, so
  `{ kind: "GET" url: any }` does not need grouping. A grouped
  `(number & not 200)` is still a full Uffda pattern.

### Handler expressions

A handler body MAY contain an expression statement `E;`. There is no
`print` keyword. Host I/O is an invocation: `(writeLine stdout "Healthy");`.

What happens to the value:

- **Discard.** `E;` runs the expression and ignores the result. Same-turn
  I/O (`write`, `writeLine`, `print`) returns `undefined` this turn. The
  statement exists for the effect.
- **Bind this turn.** `name -> E` writes a field. Nested `E` is an
  argument to another call. Check MUST reject same-turn I/O and async
  callees in assign, field init, and nested argument position.
- **Bind later.** An async callee (`http_get`) has no same-turn return.
  It is legal only as a send argument: `send self (http_get url)`. Several
  async args in one send (`send self (http_get u1) (http_get u2)`) wait
  for all, then deliver one array in argument order. The switch is `on`
  Then in the state `goto` entered this turn. There is no `{ P -> }`
  continuation on an invocation.

World binds `stdin`, `stdout`, and `stderr` as stream values in handler
scope. `(write stream V…)`, `(writeLine stream V…)`, and `(print V…)` are
host functions. One value after the stream (or after `print`) is that
value. Two or more are the array of those values.
`(print V…)` is `(writeLine stdout V…)`. They MUST NOT complete the
instance. `kith run` writes those records to process stderr as data codec
lines so process stdout stays the run result.

Uffda globals such as `length` MAY appear in expressions because they are
pure. An unknown callee MUST fail evaluation.

A send enqueues work for the dispatcher. The sender MUST NOT observe a
handler's result as a return value. If a later transition needs a result, the
receiver sends a message back.

### Terminal statements

A handler body is a synchronous prefix followed by at most one terminal.

- Assign, expression statements, `send`, constructing a child, and releasing
  a child run on this turn, in order, before any terminal. A send only
  enqueues: it MUST NOT run the receiver on this turn. Constructing a child
  only records it: it MUST NOT run that child on this turn. `send after`
  only arms a timer. An async send argument starts host work this turn and
  delivers later; `goto` after it is this turn.
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
  array MUST have exactly as many elements as prefixes. `on @Name` matches
  the registry symbol `Name` and MUST NOT match the string `"Name"`.
  `else` is matched last, as `any` with the unqualified route list. It
  MUST NOT match init.
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
  running program. The program MUST NOT read that identity. A
  [key](./glossary.spec.md#components-and-trees) is not an identity.
- Scene-constructed children MUST be created when the parent instance is
  created, one instance per construction, in declaration order. They live as
  long as the parent. A dynamic child lives until that parent releases it or
  the parent is destroyed. Construction is specified in
  [dynamic children](#dynamic-children).
- Completing a child does not destroy it and does not release it. It only
  marks the child done so it will not run another transition. `complete` is
  not destruction.
- An instance is reachable when it is the root, or its parent is reachable and
  still holds it. The parent holds each scene-constructed child, port or
  unnamed, for the parent's whole life. It holds a dynamically constructed
  child until a transition of that parent releases it, or the parent is
  destroyed. An unreachable instance MUST be destroyed, and its descendants
  with it. This is unreachability only. Failure, restart, and poison messages
  belong to the planned supervision chapter.
- Destruction runs between turns. It cancels that instance's in-flight
  transition actors, drops messages that instance has not taken, and later
  routes to it are dropped
  ([dispatch routing](./dispatch.spec.md#routing),
  [transition actors](./dispatch.spec.md#transition-actors)). It runs no
  transition, sends nothing, and offers no init. It is not a handler the
  instance matches. A `send up` already delivered into the parent's mailbox
  stays. Destruction does not pull a message back out of a mailbox that
  already has it.
- A release is a prefix statement. It records that the parent no longer holds
  a dynamically constructed direct child. It MUST NOT drop a scene-constructed
  child. A release that would do so MUST fail with a diagnostic, and the turn
  MUST NOT commit. The source spelling, including how the statement designates
  the child, is open ([open questions](#open-questions)).
- Creating an instance MUST match the start state's entry pattern, then its
  post-condition, against the initial fields before init is offered
  ([state conditions](#state-conditions)).
- After the scene tree is constructed, the runtime MUST offer each
  scene-constructed component instance init in its start state, before any
  injected message. A dynamic instance gets the same offer once, before any
  other message to it. Init is the first message in that mailbox. Sends from
  the spawning turn that route to it append after that init. Release before
  that instance runs drops the mailbox, init included.
- Init has no payload. Only `on init` matches it. Unmatched init MUST be
  dropped with no error, including at the root.

### Dynamic children

A transition MAY construct additional direct children. Construction is a
statement in the handler prefix, with assign and `send`. It records the
construction. The parent's turn does not run the new instance. Expressions
MUST NOT construct a child. The source spelling is open
([open questions](#open-questions)).

Each new instance is the same component actor as a scene-constructed one:
one parent, its own identity, its own mailbox, the start state, and init
once before any other message to it. Init is the first message in the new
mailbox. Sends from the spawning turn that route to the new instance append
after it. Release before that instance runs drops the mailbox, init included.
`on init` is still the only match for init.

The new child is unnamed in the parent's collection. It does not fill or
replace a port on the parent. Ports on the spawning parent stay the fills
the scene built. `send to` cannot address the new child. `send down` of its
kind selects it with the other direct children of that kind.

If the new kind declares ports, the construction fills them under the scene
rules in [scenes](./scenes.spec.md#constraints): known kind, declared kind,
no missing port, no duplicate port name. Nested unnamed children in that
construction follow the same rules. Check MUST fail in those cases, as it
does for a scene. Those nested children are created with the new instance
and held for that instance's life. They are not further dynamic children the
spawning parent releases one by one.

A key, when the transition supplies one, is a value that transition already
computed. It is the key a filtered `send down` matches. The program MUST NOT
read a runtime-minted identity. Omitting the key means null. Scene-constructed
children are still created with the parent, in declaration order, and their
keys stay null.

A new child is not a transition actor and not a fan-out of a message. Several
constructions in one handler are several children, recorded in statement
order. Constructions commit with the turn's machine state, and only when the
turn commits ([state conditions](#state-conditions)). The new mailboxes are
not run on the parent's turn.

### State conditions

A state MAY declare an entry pattern and a post-condition over the fields in
scope in that state. Each is an extra pattern, checked on the state change,
in addition to each field's own write pattern. Neither pattern selects the
handler. Handler choice stays the route list and the message pattern, first
match, as [matching](#matching) specifies. A transition is enabled only by
the message pattern in the current state. The patterns are pure: they MUST
NOT send, construct, release, or read a clock. Their source spellings are
open ([open questions](#open-questions)).

The check uses the order in [state-scoped fields](#state-scoped-fields). For
the proposed result of a transition (assigns applied, `goto` chosen, or the
same state if there is no `goto`), match the destination state's entry
pattern, then its post-condition, against the fields that would be in scope.
A state that does not declare a pattern passes that check. Both declared
patterns MUST match. Only then does the turn commit machine state and route
the sends and constructions it recorded, and apply the releases it recorded.

A miss MUST fail the transition with a diagnostic. Machine state stays as it
was at the start of the turn. Nothing recorded is routed. Constructions and
releases do not take effect. `complete` does not change state. The entry
pattern and the post-condition of the state that remains are still checked.
The patterns are not an exit action and not a `succeed` or `fail`.

Creating an instance checks the start state's entry pattern, then its
post-condition, against the initial fields before init is offered. A miss
MUST fail creation with a diagnostic and MUST NOT offer init.

### State-scoped fields

A field declared on the component, with `states` omitted on the field node,
stays in scope in every state. That is the field
[declaration](#declaration) specifies, including init and `name -> E`.

A field MAY instead name the states it belongs to. It is one field. Its
value persists across `goto` among those states. A transition that lands
outside the set drops it from machine state. The value that left is gone. It
is not stored beside the machine. Entering the set again initializes it with
the component-field rules: the init expression, or null when init is omitted,
then the field pattern. A miss MUST fail that initialization the way a missed
field write fails, and the transition MUST NOT commit.

A handler in a state outside the set cannot read or write the field. Check
MUST fail. A component-scoped field and a state-scoped field MUST NOT share
a name. One name is one field, not a copy per state. Entry and post-condition
patterns name only fields in scope in that state. Check MUST fail if they
name a field that is not. `complete` does not leave the state, so scoped
fields stay until a later `goto` leaves the set or the instance is destroyed.
Entering and leaving initialize or drop fields as part of the transition's
proposed machine state. They do not run another handler and do not send.
Scoped fields are not step-local data and not entry or exit actions. At
creation, fields in scope in the start state initialize with the
component-field rules, and state-scoped fields outside that set are absent,
before the creation check in [state conditions](#state-conditions).

Proposed order inside one transition:

1. The message match selects the handler. State conditions do not.
2. Assigns update component-scoped fields and any state-scoped field that
   stays in scope. Each assign MUST match that field's own pattern first.
   An assign names a field in scope in the handler's state. Check MUST fail
   otherwise. A write to a field that then leaves the set is dropped with
   that field.
3. State-scoped fields that are newly in scope initialize.
4. Fields that leave the set are absent.
5. The destination state's entry pattern is matched against the fields that
   would be in scope. Then its post-condition is matched against those same
   fields.
6. The turn commits, routes sends and constructions, and applies releases
   only if both match. A miss fails as
   [state conditions](#state-conditions) specifies.

## Syntax tree

The grammar MUST project source to this tagged data shape. The interpreter MUST
accept the same shape constructed without parsing. Serialized trees, program
artifacts, and snapshots MUST use the Uffda data codec, not JSON.
Uffda pattern and expression subtrees keep Uffda's own AST kinds (`over`,
`variable`, `invocation`, `number`, `resolve`, …).

- `{ kind: "program", imports: [Import], exports: [Export], components: [Component], scenes: [Scene] }`
- `{ kind: "import", moduleUrl: string, names: [string] }`
- `{ kind: "export", name: string }`
- `{ kind: "component", name: string, fields: [Field], ports: [Port], states: [State] }`
- `{ kind: "field", name: string, pattern: Pattern, init: Expression, states: [string] }`
- `{ kind: "port", component: string, name: string }`
- `{ kind: "scene", name: string, root: Instance }`
- `{ kind: "instance", component: string, key: string | null, children: [Instance] }`
- `{ kind: "state", name: string, start: boolean, named: boolean, handlers: [Handler], entry: Pattern, post: Pattern }`
- `{ kind: "handler", pattern: Pattern, item: Pattern | null, route: ["up" | "down" | "self" | "init"], body: [Statement] }`
- `{ kind: "assign", field: string, expression: Expression }`
- `{ kind: "sendUp", message: Expression | Sequence, after: Expression | null }`
- `{ kind: "sendTo", port: string, message: Expression | Sequence, after: Expression | null }`
- `{ kind: "sendDown", component: string | null, message: Expression | Sequence, after: Expression | null }`
- `{ kind: "construct", component: string, key: Expression | null, children: [Instance] }`
- `{ kind: "send", message: Expression | Sequence, after: Expression | null }`
- `{ kind: "complete", message: Expression | Sequence }`
- `{ kind: "expr", expression: Expression }`
- `{ kind: "sequence", items: [Expression] }`
- `{ kind: "goto", state: string }`

Exactly one state in a component MUST have `start: true`. Unnamed `start { }`
MUST project `name: "Start"` and `named: false`. `start Name` and
`state Name` MUST project `named: true`. Field `pattern` and
`init` MAY be omitted when the field is untyped or uninitialized; a missing
`init` is `undefined` until a transition assigns it. Field `states` MAY be
omitted. Omitted means the field is in scope in every state, so a tree that
exists today stays valid. A present `states` lists the state names it belongs
to, including the implicit `Start` of an unnamed start. The list MUST name
at least one state of this component. An unknown name or an empty list MUST
fail check. State `entry` and `post` MAY be omitted. Omitted means that state
does not declare the pattern. They are tree properties, not a source spelling.
The root instance's `key` MUST be `null`. A port fill's `key` MUST be the
port name. A scene unnamed child's key MUST be null. A dynamic child's key
MUST be the value the transition supplied, or null. The instance node's
`key` field records a port name or null. It is not the glossary Key: every
scene-constructed child's glossary Key is null, including a port child, and
a dynamic child's glossary Key is that supplied value or null.
`construct.key` is the expression for that glossary Key, not a port name.
Omitted or null means a null key. `construct.children` MAY be omitted.
When present they are instance nodes for the new child's own port fills and
nested unnamed children. `sendDown.component`
MUST be `null` when the send is `send down any`. A filtered `send down`
records `filter: Pattern` on `sendDown` and MUST set `component` to `null`.
Kind-only and `send down any` omit `filter` and keep the shape above.
A release statement has no decided tree tag or child-designation fields.
It MUST NOT be projected as `assign`, `send`, `goto`, or `complete`.
`handler.item` MAY be omitted
when the transition matches one message. `handler.route` is a list. `on down`
MUST project `["down"]`. `on up or self` MUST project `["up", "self"]`.
Unqualified `on` and `else` MUST project `["up", "down", "self"]`. `on init`
MUST project `["init"]` and pattern `any`. An omitted `route` on a
constructed tree is the unqualified list. A send or complete of
two or more values MUST project `{ kind: "sequence", items: [...] }`. One
value MUST project that expression, not a sequence of length one. An
expression statement MUST project `{ kind: "expr", expression }`. A
handler Then of several prefixes MUST project `{ kind: "then", patterns }`.
One prefix MUST project that pattern, not a Then of length one.
`handler.item` MAY still be set on a constructed tree; source uses Then.

## Example

Injecting the symbol `Ping` and `7` at `Ping` MUST `send to child`, Child
MUST `send up` the symbol `Pong` and `7`, and `Ping.heard` MUST be `7`.

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
  handler `{`. A non-empty object may still be a later prefix, and an empty
  `{}` stays the body. Kith allows
  comma-optional Over entries that still project Uffda Over.
- **`send to` / `send down` / `send up` / `send self`.** A named port is not
  a kind. `to` names the port; `down` names the kind, every child, or a
  filter; `up` is the ancestor chain; `self` is this instance on a later
  turn. A missing route would look
  like a parent sending down.
- **One terminal, last.** Several `goto`s would leave the next state's
  meaning ambiguous. Running statements after `goto` or `complete` would
  hide that the turn already ended. Sends stay later-turn, so `send` then
  `goto` is the only order that needs to exist.
- **The filter sees the parent's record.** Matching a child's fields would
  read another instance. The parent already holds kind, port name, and key.
- **Destruction is not a message.** A handler on destroy would be supervision.
  Unreachability drops the instance between turns and does not run it.
- **State conditions do not select the handler.** The message pattern stays
  the only enablement. The entry pattern and the post-condition check the
  fields the turn would commit.

## Open questions

- **Entity-like parts.** Whether a component MAY have unnamed owned parts that
  are not independently dispatched actors. Dynamic children are actors. This
  question stays open for parts that are not.
- **How higher languages specialize a component** (restriction of messages,
  durable identity, a single mailbox as a consistency boundary) without
  introducing is-a. Deferred until a language such as Coleslaw maps onto Kith.
- **`send up P V`.** Filtering ancestors with a pattern is not in this slice.
  The filtered `send down` does not decide it.
- **Spelling.** Filter, construct, release, entry pattern, and post-condition
  need source spellings. They are new forms beside [declaration](#declaration),
  not silent edits to `field`, `on`, or `send down Kind`. How a release
  designates the child is part of that open spelling.
