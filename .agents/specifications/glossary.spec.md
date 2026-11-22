# Glossary

This chapter defines Kith's vocabulary. Every chapter, and every keyword in the
language, uses these terms with these meanings. Kith is named for kith, not kin:
the company you keep by acquaintance, not by descent.

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be interpreted
as described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Using the glossary

- A chapter MUST use a term from this glossary only with the meaning given here.
- A keyword that names one of these concepts MUST be the term itself (for
  example `component`, `scene`, `send`, `to`, `else`, `goto`, `complete`,
  `after`, `up`, `down`, `self`, `init`, `or`).
- A new concept MUST be added here before a chapter relies on it.
- Kith MUST NOT take Coleslaw's domain terms (aggregate, manager, reactor,
  controller, command, event as Coleslaw defines them). Those belong to
  languages above Kith.

## Structure

- **Program**: the set of modules that make up one Kith application.
- **Module**: one Kith source file.
- **Built-in module**: a module named `kith:` plus one lowercase name. The
  host registers its declaration with the Uffda resolver. It is not
  downloaded and not locked. An import map does not rewrite it. Its
  functions and rules come from that declaration. Its components are host
  drivers.
- **Rule**: a pattern exported by a module. After import it is a pattern
  name, in the same scope as `number` and `string`. Matching it does not
  send and does not read a clock.
- **Package**: modules published together under one name and version, imported
  as Uffda's packages are.
- **Root**: the instance that has no parent. A running program has one root:
  the top-level component instance of its exported scene.
- **Scene**: a named instance tree that constructs component kinds, fills
  ports, and may add unnamed children.
- **Port**: a named slot on a component for one child (`name: Kind;`).
  `Kind` is a looks-like selector, not an is-a bind on the declared
  component. A mock matches when it satisfies that pattern. The scene fills
  it. `send to name` addresses that instance.

## Uffda

- **Pattern**: a Uffda pattern. Messages, fields, and parameters are matched by
  patterns. A value has a pattern when the pattern matches it, and the value
  used is the value the pattern produces.
- **Expression**: a Uffda expression. Uffda globals compute values and have
  no side effects. Kith MAY intercept a host callable (`rand`, `writeLine`,
  `http_get`) while evaluating an expression AST; that is not a Uffda global.
  Same-turn I/O returns `undefined`. Async I/O has no same-turn return: it is
  a send payload whose result arrives as a message.
- **Stream**: a capability value for a byte stream (`stdin`, `stdout`,
  `stderr`). World binds the defaults in handler scope. A test or parent MAY
  bind a different stream. `(write stream V)` and `(writeLine stream V)` take
  the stream as the first argument.
- **Stdlib**: named host functions plus the types of the handles they accept.
  They are not keywords. `(print V…)` is `(writeLine stdout V…)`.

## Components and trees

- **Component**: a named, composable kind with a state machine, optional
  fields, and optional ports. Instances of a component are constructed by a
  scene, or by a parent's transition. The instance is the same either way.
  A component is not a subtype of another component.
- **Instance**: one running component, with its own identity, state,
  children, and mailbox.
- **Destroyed**: an instance reachability has dropped
  ([components](./components.spec.md#identity-and-lifetime)). Destruction is
  not a state and not a message. The instance runs no transition, sends
  nothing, and is offered no init. Its descendants are destroyed with it.
- **Child**: an instance owned by another instance. Ownership is has-a. A
  dynamic child is one a parent's transition constructs. It is unnamed, and
  it is the same kind of instance as a scene-constructed child. It lives
  until that parent releases it or the parent is destroyed.
- **Key**: a value the parent holds for a direct child, separate from the
  selector pattern and from its port name. A filtered `send down` may match
  it. The filter MUST NOT treat this value, or a component name, as an
  equality check on the child's declared kind.
  A scene-constructed child's key is null, whether or not it fills a port.
  The scene instance node's `key` field is that port name, or null; it is
  not this value. A dynamic child's key is the value the constructing
  transition already computed, or null when that transition supplies none.
  It is not a runtime-minted identity, and the program MUST NOT read one.
- **Parent**: the instance that owns a child. An instance other than the root
  has exactly one parent.
- **Tree**: the has-a ownership graph of a running program. It is a tree: no
  instance has two parents, and no instance is its own ancestor.

## Messages and routing

- **Message**: a value, or a sequence of values packed as an array, sent from
  one instance to another and matched as a Uffda pattern at the receiver.
  One send argument is that value; two or more arguments are the array.
  Messages are the only communication that causes behavior.
- **Send**: the act of placing a message on a route. Sending does not wait for
  a handler. The keyword is `send`.
- **To**: a route to a named port (`send to`).
- **Up**: a route toward the parent, and then toward the root if not consumed
  (`send up`). `on up` matches only that route at the receiver.
- **Down**: a route toward direct children (`send down Pattern V` or
  `send down any V`). The selector is a pattern. A component name there is
  a looks-like match, not declared-kind equality. `any` matches every direct
  child. A filter matches a pattern the parent holds and MUST NOT require
  the child's declared kind to equal a named component. The message is the
  value after the selector. `on down` matches a `send to` / `send down` to
  this instance,
  and an inject. It does not match `send self` or init.
- **Self**: a route to this instance (`send self`). Taken on a later turn.
  A handler matches it when its route list includes `self`. Unqualified
  `on` is `up or down or self`. `on down` does not include `self`.
- **Or**: joins route names on a handler (`on up or down`). It is not
  Uffda pattern `|`.
- **Init**: a one-time offer to each scene-constructed component instance
  after the tree is built. A dynamic instance gets the same offer once,
  before any other message to it. It has no payload. Only `on init` matches.
  Unmatched init is dropped.
- **After**: a delay on a send (`send after Delay`). Delay is seconds. The
  send is delivered when the dispatcher clock reaches the due time. It is
  not a blocking wait.
- **Rand**: a Kith host callable `(rand Lo Hi)` that produces a number in
  that range from World RNG. Inclusive integers when both bounds are
  integers. It is not a Uffda global.
- **Consume**: a matched transition that handles a message so the message does
  not continue on its route.

## Machines

- **State**: a named condition a machine is in.
- **Start state**: the state a machine begins in. Unnamed `start { }` is
  not a `goto` target; `start Name` is.
- **Transition**: the handling of one enabling message in one state: matching
  a pattern, computing with expressions, changing machine state, sending
  messages, constructing or releasing a child, or spawning work. A transition
  is the only behavior.
- **Machine state**: the state name, plus the values of the fields in scope
  in that state.
- **Goto**: a component statement (`goto Name`) that enters another declared
  state of this instance. It is a terminal: at most one per handler, last
  if present, exclusive with `complete`.
- **Complete**: finishing a component (`complete V`). The payload is sent
  up (except at the root). Complete is a terminal: at most one per
  handler, last if present, exclusive with `goto`. It is not a machine
  state. The instance stays in the state that ran `complete` and MUST NOT
  run another transition.

## Dispatch

- **Actor**: a unit of concurrent execution with a mailbox. Every component
  instance is an actor. An in-flight transition MAY be an actor.
- **Mailbox**: the ordered queue of messages an actor has not yet taken.
- **Dispatcher**: the runtime that routes messages, schedules ready actors, and
  is the parallelization mechanism. It is not a declaration in the program.
- **Uffda runtime**: the `uffda-runtime` crate: languages, patterns,
  expressions, values. It does not schedule Kith actors.
- **Kith runtime**: the interpreter in the `kith` binary that owns instances
  and turns, and calls the Uffda runtime to match and evaluate.
- **Ready**: an actor that has at least one message in its mailbox, or a
  transition actor whose work can proceed.

## Host

- **Print**: the host function `(print V…)`, the same as
  `(writeLine stdout V…)`. It is not a statement and MUST NOT complete the
  instance. It is not the `Stderr` component.
- **Frozen argv**: the program arguments captured at start, after `kith run`
  flags and the program path. Later reads see that copy.
- **Frozen env**: the process environment captured at start. Later reads see
  that copy and MUST NOT re-read the operating system.
