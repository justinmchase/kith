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
  reached only through their parent: `send to` a port, or `send down`.

### Behavior

- A component MUST declare exactly one start state.
- Choosing what happens MUST be done by a transition. Expressions MUST NOT send
  messages or change machine state.
- A transition MUST be deterministic: the same machine state and the same
  matched message MUST produce the same field changes, the same sends, the
  same children constructed or released, and the same entry-pattern and
  post-condition matches.
- A transition MAY send zero or more messages, change fields, evaluate
  expression statements, complete, and change the machine's state. It MUST
  NOT wait by blocking the dispatcher. The
  first executable slice MUST NOT spawn or destroy children during the turn.
  Scene-constructed children exist for the parent's lifetime. A transition MAY
  record a release of a dynamic child. Destruction of an unreachable instance
  runs between turns. A component MAY `complete V`.

### Routing

- A send MUST name a direction: `send to` a port, `send down`,
  `send up`, or `send self`. Routing is explicit only: there is no capture
  phase, and an unhandled message MUST NOT travel up unless the send was
  `send up`. A send with no route is invalid; self is not implied.
- `send up` offers the message to the parent, then to that parent's parent,
  until a transition consumes it or the root does not. An unmatched send up at
  the root MUST be dropped with no error.
- `send to name V` offers `V` to the child that fills port `name`. An unknown
  port MUST fail that send with a diagnostic.
- `send down V` offers `V` to every direct child, a port child and an unnamed
  child together. `send down P V` is not a form. The parent does not filter
  children, does not compare component names, and does not select or care
  which children take the message. It MUST NOT skip a generation.
- A child whose current state has a matching `on down` handles `V`. A child
  with no matching `on down` ignores that offer. That is not a failed send,
  and it MUST NOT be forwarded to that child's children. If no child handles
  `V`, including when the parent has no direct children, that is not an error.
- `send to name V` that the addressed child does not handle MUST fail that
  send with a diagnostic. The failure MUST NOT be forwarded to that child's
  children.
- Several children that handle one `send down` MUST each receive one send,
  independently.
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
  by a scene.
- `field name: pattern = expression` is local state. Every write MUST
  match `pattern` as a Uffda scalar value: the init and `name -> E`.
  A miss MUST fail that write with a diagnostic.
  `= expression` MAY be omitted; a missing init is **null**. `number?`
  matches null or a number and MUST NOT match a string: Uffda Maybe that
  skips a present non-null value is not a match for a stored field.
  The value used is the value that matched.
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
  `send to` of a shape the child does not handle fails that send. A
  `send down` of a shape a child does not handle is ignored by that child.
  A child sending a parent's request shape continues up.
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

### Sending

- `send to name V…` sends the message to the named port.
- `send down V…` offers the message to every direct child. A child with no
  matching `on down` ignores it. A component name before the message is not
  a selector.
- `send up V…` sends the message toward the parent.
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

- Assign, expression statements, and `send` run on this turn, in order,
  before any terminal. A send only enqueues: it MUST NOT run the receiver
  on this turn. `send after` only arms a timer. An async send argument
  starts host work this turn and delivers later; `goto` after it is this
  turn.
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
  Several children offered one `send down` are independent offers.
- An unmatched message that was travelling up MUST continue to the next
  ancestor, and MUST be dropped with no error if the root does not consume it.
- An unmatched message that was sent to this instance MUST fail that send
  with a diagnostic. It MUST NOT be forwarded to this instance's children.
- An unmatched `send down` is ignored by that child. It MUST NOT fail the
  send and MUST NOT be forwarded to that child's children.
- This match selects the handler. An entry pattern and a post-condition do
  not ([state conditions](#state-conditions)).

### Identity and lifetime

- Each instance MUST have an identity unique among instances of its kind in the
  running program.
- Scene-constructed children MUST be created when the parent instance is
  created, one instance per construction, in declaration order. They live as
  long as the parent. Completing a child does not destroy it and does not
  release it; it only marks it done so it will not run another transition.
- A dynamic child lives until a transition of that parent releases it, or
  the parent is destroyed. The parent holds it in the same child collection
  as a scene-constructed child.
- An instance is reachable when it is the root, or its parent is reachable
  and still holds it. An unreachable instance MUST be destroyed, and its
  descendants with it. Destruction runs between turns. It MUST cancel that
  instance's in-flight transition actors, drop messages that instance has
  not taken, and drop later routes to it. Cancelled work MUST NOT enqueue a
  successful outcome. It MUST run no transition, send nothing, and offer no
  init. It is not a handler the instance matches.
- A `send up` already delivered into a mailbox MUST stay. Destruction MUST
  NOT pull a message back out of a mailbox that already has it.
- A release records that the parent no longer holds each dynamic child its
  pattern matches. The pattern matches the record the parent holds: `port`
  is the port name, or null when the child is unnamed, and `key` is the
  value the parent holds for that child. A scene-constructed child holds
  null. A dynamic child holds the key its constructing transition computed,
  or null when that transition omitted one.
  It MUST NOT see the child's fields, machine state, or mailbox, and it
  MUST NOT compare the child to a named component. A release MUST NOT drop
  a scene-constructed child. A release that would do so MUST fail with a
  diagnostic and MUST NOT drop a child. Zero matches MUST fail that
  release. The source spelling is open
  ([open questions](#open-questions)). Collection is unreachability only.
- Creating an instance MUST match the start state's entry pattern against
  the initial fields before init is offered
  ([state conditions](#state-conditions)). A miss MUST fail creation and
  MUST NOT offer init. Creation does not leave a state, so it does not run
  the start state's post-condition.
- After the scene tree is constructed, the runtime MUST offer each
  component instance init in its start state, before any injected message.
  Init has no payload. Only `on init` matches it. Unmatched init MUST be
  dropped with no error, including at the root.
- The first slice MUST NOT run a child it constructs. Construction is
  recorded on the turn and the new instance runs afterward.

### Dynamic children

A transition MAY construct an unnamed child. The source spelling is open
([open questions](#open-questions)). The tree is
`{ kind: "construct", component: string, key: Expression | null, children: [Instance] }`.

- The statement records the construction. The parent's turn MUST NOT run the
  new instance.
- The new instance is a component actor with one parent, its own mailbox,
  and the start state. Init is offered once, and it is the first message in
  that mailbox. Sends from the spawning turn append after that init. `on
  init` is the only match for init.
- The new child is unnamed. It does not fill or replace a parent port.
  `send to` MUST NOT address it. `send down V` still offers `V` to it with
  every other direct child. A child with no matching `on down` ignores that
  offer.
- A key, when the transition supplies one, is a value that transition
  already computed. Omitting it means null. A release MAY match that key.
  The program MUST NOT read a runtime-minted identity.
- If the new component declares ports, the construction MUST fill each one
  with an instance of the declared component. A missing port, a duplicate
  port name, or a different component MUST fail. Nested children are created
  with the new instance.
- A release in the same turn, before the new instance runs, MUST drop it,
  init included, so it does not run. Expressions MUST NOT construct.

### State conditions

A state MAY declare an entry pattern and a post-condition over the fields in
scope in that state. Each is an extra pattern, in addition to each field's
own write pattern. Neither pattern selects the handler. Handler choice
stays the route list and the message pattern, first match, as
[matching](#matching) specifies. Each assign MUST match that field's own
pattern before the entry pattern is tried.

The entry pattern is the check of the state the turn would commit: the
destination of `goto`, or the same state when the turn does not leave. The
post-condition is the check before leaving the current state. It is not a
check of the state being entered.

On every turn, match the destination state's entry pattern against the
fields after the handler's assignments, before the turn commits. A state
that does not declare an entry pattern passes that check. On a turn that
would leave the state, after that entry pattern matches, the post-condition
of the state being left runs on those same fields, before `goto` commits.
A state that does not declare a post-condition passes that check. Only then
does the turn commit machine state and route the sends and constructions it
recorded, and apply the releases it recorded.

A miss MUST fail the transition with a diagnostic. Machine state stays as it
was at the start of the turn, including every field the handler assigned.
Nothing recorded is routed. Constructions and releases do not take effect.
A post-condition on a state the turn only enters does not run on that
entering turn. A turn that does not leave its state does not run that
state's post-condition. `complete` does not leave the state, so it does not
run the post-condition. It still matches the entry pattern of the state that
remains. The patterns are not an exit action and not a `succeed` or `fail`.

The patterns are pure. They do not send, construct, or read a clock.

Creating an instance MUST match the start state's entry pattern against the
initial fields before init is offered. A miss MUST fail creation with a
diagnostic and MUST NOT offer init. Creation does not leave a state, so it
does not run the start state's post-condition. This check does not select a
handler.

The source spelling stays open ([open questions](#open-questions)). The tree
properties are `entry` and `post`.

`Gate` keeps `n` in scope. Closed's `post` is `{ n: 0 }`. Open's `post` is
`{ n: 1 }`. The `post` line in this program is that tree property, not a
chosen source spelling. The example does not leave Open. Open's post does
not run on the turn that only enters Open.

The test harness delivers `@Open` and `@Stuck` to Gate as `down`. Gate keeps
`on down` for those messages. Closed's post runs because those turns would
leave Closed.

```kith
component Counter {
  field seen: number = 0;
  start {
    on down @Tick {
      seen -> 1;
    }
  }
}

component Gate {
  field n: number = 0;
  tick: Counter;

  start Closed {
    post { n: 0 }
    on down @Open {
      send to tick @Tick;
      goto Open;
    }
    on down @Stuck {
      n -> 1;
      send to tick @Tick;
      goto Open;
    }
  }

  state Open {
    post { n: 1 }
  }
}

export scene Main {
  Gate() {
    tick = Counter();
  }
}
```

`@Open`, delivered to Gate as `down`, would leave Closed. After the handler's
assignments `n` is still `0`, so Closed's `post { n: 0 }` matches before
`goto` commits. The turn commits: Gate is `Open`, and `@Tick` is routed, so
`seen` is `1`. Open's `post { n: 1 }` does not run on that entering turn.

`@Stuck`, delivered the same way, assigns `n` to `1` and then would leave
Closed. Closed's `post { n: 0 }` sees `n` as `1` and misses, so the turn does
not commit: Gate stays `Closed`, `n` stays `0`, and `@Tick` is not routed.

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
- `{ kind: "field", name: string, pattern: Pattern, init: Expression }`
- `{ kind: "port", component: string, name: string }`
- `{ kind: "scene", name: string, root: Instance }`
- `{ kind: "instance", component: string, key: string | null, children: [Instance] }`
- `{ kind: "state", name: string, start: boolean, named: boolean, handlers: [Handler], entry: Pattern | null, post: Pattern | null }`
- `{ kind: "handler", pattern: Pattern, item: Pattern | null, route: ["up" | "down" | "self" | "init"], body: [Statement] }`
- `{ kind: "assign", field: string, expression: Expression }`
- `{ kind: "sendUp", message: Expression | Sequence, after: Expression | null }`
- `{ kind: "sendTo", port: string, message: Expression | Sequence, after: Expression | null }`
- `{ kind: "sendDown", component: string | null, message: Expression | Sequence, after: Expression | null }`
- `{ kind: "construct", component: string, key: Expression | null, children: [Instance] }`
- `{ kind: "release", filter: Pattern }`
- `{ kind: "send", message: Expression | Sequence, after: Expression | null }`
- `{ kind: "complete", message: Expression | Sequence }`
- `{ kind: "expr", expression: Expression }`
- `{ kind: "sequence", items: [Expression] }`
- `{ kind: "goto", state: string }`

Exactly one state in a component MUST have `start: true`. Unnamed `start { }`
MUST project `name: "Start"` and `named: false`. `start Name` and
`state Name` MUST project `named: true`. State `entry` and `post` MAY be omitted.
Omitted means that state does not declare that pattern. They are tree
properties, not a source spelling. Field `pattern` and
`init` MAY be omitted when the field is untyped or uninitialized; a missing
`init` is `undefined` until a transition assigns it. The root instance's `key`
MUST be `null`. An unnamed child's `key` MUST be `null`. `sendDown.component`
MUST be `null`. `send down` does not name a component and does not carry a
filter. A release statement's tree is `{ kind: "release", filter: Pattern }`.
The pattern matches `port` and `key` as
[identity and lifetime](#identity-and-lifetime) defines. `handler.item` MAY be omitted
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
  a kind. `to` names the port; `down` offers the message to every direct
  child, and a child that does not match `on down` ignores the offer; `up`
  is the ancestor chain; `self` is this instance on a later turn. A missing
  route would look
  like a parent sending down.
- **Unreachability, not a destructor.** A handler on destroy would send and
  change state while the instance is already going away. Dropping the
  instance between turns cancels work and leaves mail that was already
  taken where it was delivered.
- **A release sees what the parent holds.** Matching a child's fields,
  machine state, or mailbox would read another instance. The parent holds
  the port name and the key.
- **State conditions do not select the handler.** The message pattern stays
  the only enablement. The entry pattern checks the state the turn would
  commit. The post-condition checks the fields of the state the turn would
  leave, after that handler's assignments and before `goto` commits. The
  entry pattern is a condition on the state change, not a guard on `on`.
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
- **Release spelling.** The source spelling of a release, including how it
  designates the child, stays open. The tree is `{ kind: "release", filter }`.
- **Construct spelling.** The source spelling of constructing a child stays
  open. The tree is `{ kind: "construct", component, key, children }`.
- **Post-condition spelling.** The source spelling of a post-condition stays
  open. The form the tree names is `post` on the state
  ([state conditions](#state-conditions)). It is not a silent edit to
  `field` or `on`.
- **Entry spelling.** The source spelling of an entry pattern stays open.
  The form the tree names is `entry` on the state
  ([state conditions](#state-conditions)). It is not a silent edit to
  `field`, `on`, or `post`.
