# Dispatch

This chapter defines the dispatcher: the runtime that routes messages, treats
components and in-flight transitions as actors, and parallelizes independent
work. Component sends are defined in [components](./components.spec.md).
Terms are defined in the [glossary](./glossary.spec.md).

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be interpreted
as described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Definitions

- **Component actor**: the actor for one component instance. Its mailbox
  holds messages routed to that instance.
- **Transition actor**: the actor for one in-flight transition whose work is
  not yet complete, such as a join waiting on an async send.
- **Schedule**: the dispatcher's choice of which ready actor runs next.
- **Turn**: one actor taking one message (or one transition-actor step),
  running at most one transition of a component actor, and enqueuing the
  sends, spawns, and child constructions or releases that transition produced.

## Axioms

- The dispatcher is the only scheduler. Programs do not create threads, locks,
  or callbacks to interleave work.
- A component actor processes one turn at a time. Its machine state is not
  observed mid-turn by another actor.
- Independent ready actors MAY run at the same time.
- A message is delivered at least once to the mailbox it was routed to, unless
  the instance has been destroyed.
- Completing a transition actor sends its outcome to the component actor that
  spawned it, as a message.

## Constraints

### Serialization per instance

- Two turns of the same component actor MUST NOT overlap.
- A transition that needs concurrent work MUST spawn transition actors or
  children; it MUST NOT run that work on the component actor's turn beyond
  computing the spawn.

### Parallelism

- The dispatcher MUST be allowed to run ready actors of different instances in
  parallel.
- Sibling component actors MUST be allowed to run in parallel with each
  other.
- Parallelism MUST NOT change a single actor's per-message semantics. It only
  interleaves independent actors.

### Isolation

- An actor MUST NOT read or write another actor's machine state.
- The only inputs to a component turn are the instance's machine state and the
  message being taken.
- The only outputs of a component turn are the next machine state, messages
  sent, host stream writes, actors spawned or joined, and children
  constructed or released.

### Fairness and variation

- The dispatcher SHOULD NOT starve a ready actor indefinitely.
- The order in which ready actors run MUST NOT be part of a program's meaning.
- A checker MUST be able to choose any order of ready actors.

## Mechanism

### Serial schedule (first slice)

- The first executable dispatcher MUST run one ready actor at a time from a
  FIFO ready queue.
- Parallel execution of independent actors remains allowed by this chapter; it
  MUST NOT be required of the first slice. The queue is the extension point.
- Transition actors MUST NOT be required of the first slice. Turns are
  synchronous: match, evaluate, apply field changes, enqueue sends, and
  record child constructions and releases.

### Component actors

- Creating an instance creates its component actor and an empty mailbox, and
  puts the machine in its start state. An instance a parent's transition
  constructs after the scene tree exists is that same kind of actor.
- After the scene tree is constructed, the dispatcher offers each
  scene-constructed component instance init. Only `on init` matches.
  Unmatched init is dropped. This MUST happen before any injected message.
- A dynamically constructed instance is offered init once, as the first
  message in its mailbox, before any other message to it. Sends the spawning
  turn routes to it append after that init. The parent's turn does not run
  the new instance. Releasing it before it runs drops the mailbox, init
  included.
- A send that routes to an instance appends the message to that instance's
  mailbox and marks the actor ready.
- A turn dequeues one message, matches it in the current state, and if a
  transition matches, runs that transition to completion of its synchronous
  work (expressions, field changes, state change, recording sends, spawns,
  and child constructions or releases).
- Sends recorded in a turn MUST be routed after the turn's state change is
  visible, including children constructed or released on that turn, so a
  message to self (`send self V`) is taken on a later turn
  and matched in the state after that turn's terminal `goto`, if any.

### Transition actors

- A transition MAY leave work for the dispatcher that proceeds off the
  component turn, such as a delayed or async send.
- A transition actor has a mailbox only for the completions it needs. It MUST
  NOT be a second way to address the component.
- When a transition actor completes, the dispatcher MUST enqueue an outcome
  message on the spawning component actor. The component matches that message
  in the state it is in when the outcome arrives, which MAY be a different
  state than the one that spawned the work.
- The reachability rule in
  [components](./components.spec.md#identity-and-lifetime) is what destroys an
  instance. Destroying an instance MUST cancel its in-flight transition
  actors. Cancelled work MUST NOT enqueue a successful outcome after
  cancellation.

### Routing

- The dispatcher performs send to, send down, send up, and send self as
  [components](./components.spec.md#routing) specifies. Routing chooses a
  mailbox; it does not run a transition.
- The reachability rule in
  [components](./components.spec.md#identity-and-lifetime) is what destroys an
  instance. Routing to a destroyed instance MUST drop the message.

## Why this design

- **Actors rather than a call stack.** Calls would nest transitions of
  different instances on one thread and hide parallelism. Mailboxes make
  "one instance at a time" and "many instances at once" the same mechanism.
- **Transitions as actors, not only components.** Delayed and async sends
  are in-flight work the dispatcher owns. Children remain the structured
  has-a tree.
- **Outcomes as messages.** Returning to the middle of a turn after await would
  reintroduce a stack and make machine state visible mid-transition. Completing
  work by a new message keeps every change a transition in a known state.

## Open questions

- **At-least-once vs exactly-once** delivery across a crash or a move.
- **Mailbox bounds.** Whether a mailbox may grow without limit, and what
  happens when it cannot.
- **Priority.** Whether some messages skip ahead in a mailbox.
- **Reentrancy to self.** Whether a send to self is always a later turn
  (as above) or MAY be taken before other mail.
- **How much of a transition is synchronous.** Whether `send after` is the
  only way to wait, or a transition body MAY pause and resume.
- **Supervision.** Failure, restart, and poison messages belong to the planned
  `supervision` chapter. Collection is unreachability only.
