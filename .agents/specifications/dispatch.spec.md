# Dispatch

This chapter defines the dispatcher: the runtime that routes messages, treats
components and in-flight transitions as actors, and parallelizes independent
work. Component sends are defined in [components](./components.spec.md).
Workflow joins and fan-out are defined in [workflows](./workflows.spec.md).
Terms are defined in the [glossary](./glossary.spec.md).

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be interpreted
as described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Definitions

- **Component actor**: the actor for one component or workflow instance. Its
  mailbox holds messages routed to that instance.
- **Transition actor**: the actor for one in-flight transition whose work is
  not yet complete, such as fan-out work or a join waiting on children.
- **Schedule**: the dispatcher's choice of which ready actor runs next.
- **Turn**: one actor taking one message (or one transition-actor step),
  running at most one transition of a component actor, and enqueuing the sends
  and spawns that transition produced.

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
- Fan-out children and sibling transition actors MUST be allowed to run in
  parallel with each other.
- Parallelism MUST NOT change a single actor's per-message semantics. It only
  interleaves independent actors.

### Isolation

- An actor MUST NOT read or write another actor's machine state.
- The only inputs to a component turn are the instance's machine state and the
  message being taken.
- The only outputs of a component turn are the next machine state, messages
  sent, host stream writes, and actors spawned or joined.

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
  synchronous: match, evaluate, apply field changes, enqueue sends.

### Component actors

- Creating an instance creates its component actor and an empty mailbox, and
  puts the machine in its start state.
- After the scene tree is constructed, the dispatcher offers each component
  instance init. Only `on init` matches. Unmatched init is dropped. This
  MUST happen before any injected message.
- A send that routes to an instance appends the message to that instance's
  mailbox and marks the actor ready.
- A turn dequeues one message, matches it in the current state, and if a
  transition matches, runs that transition to completion of its synchronous
  work (expressions, field changes, state change, recording sends and spawns).
- Sends recorded in a turn MUST be routed after the turn's state change is
  visible, so a message to self (`send self V`) is taken on a later turn
  and matched in the state after that turn's terminal `goto`, if any.

### Transition actors

- A transition MAY spawn transition actors for work that proceeds off the
  component turn: each fan-out item, a join, or other declared waiting work.
- A transition actor has a mailbox only for the completions it needs. It MUST
  NOT be a second way to address the component.
- When a transition actor completes, the dispatcher MUST enqueue an outcome
  message on the spawning component actor. The component matches that message
  in the state it is in when the outcome arrives, which MAY be a different
  state than the one that spawned the work.
- Destroying an instance MUST cancel its in-flight transition actors. Cancelled
  work MUST NOT enqueue a successful outcome after cancellation.

### Routing

- The dispatcher performs send to, send down, send up, and send self as
  [components](./components.spec.md#routing) specifies. Routing chooses a
  mailbox; it does not run a transition.
- Routing to a destroyed instance MUST drop the message.

### Clustering

- A workflow's component actor MAY run on any process in the cluster. The
  dispatcher on that process owns the actor while it is there.
- Moving an actor MUST NOT reorder messages already in its mailbox, and MUST
  NOT duplicate a completed turn. How that is achieved is
  [workflows](./workflows.spec.md#clustered-execution).

## Why this design

- **Actors rather than a call stack.** Calls would nest transitions of
  different instances on one thread and hide parallelism. Mailboxes make
  "one instance at a time" and "many instances at once" the same mechanism.
- **Transitions as actors, not only components.** If only instances were
  actors, every concurrent piece of work would have to be a declared child.
  Fan-out of a stream would force a child kind per item even when the parent
  has no has-a relationship beyond the join. Transition actors are the
  in-flight work of a step; children remain the structured has-a tree.
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
- **How much of a transition is synchronous.** Whether spawning is the only
  way to wait, or a transition body MAY be a sequence of steps each run as
  transition actors.
- **Supervision.** Failure, restart, and poison messages belong to the planned
  `supervision` chapter.
