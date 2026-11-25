---
id: components-024
title: A post-condition must match before the turn commits
spec_ref: .agents/specifications/components.spec.md#state-conditions; .agents/specifications/components.spec.md#matching; .agents/specifications/components.spec.md#identity-and-lifetime
---

# Post-conditions

## Requirement

Preconditions:

- A state may declare `post`, a pattern over the fields in scope. The source
  spelling stays open. Omitting `post` means the state declares none.
- Handler choice is the route list and the message pattern. The
  post-condition does not select a handler.

Expected behavior:

- For the proposed result of a transition, match the entry pattern when that
  state declares one, then the post-condition. A state that does not declare
  an entry pattern passes that check. Both MUST match before the turn commits
  machine state or routes recorded sends and constructions.
- A miss MUST fail the transition with a diagnostic that says
  `post-condition`. Machine state MUST stay as it was at the start of the
  turn. Nothing recorded is routed. A construction recorded in that turn
  MUST NOT be created, and its init MUST NOT run.
- `complete` does not change state. The post-condition of the state that
  remains MUST still be checked. A miss MUST leave the instance not
  completed.
- `goto` checks the destination state's post-condition. A miss MUST leave
  the machine in the state it started the turn in.
- The pattern is pure. It does not send, construct, or read a clock.
- Creation MUST check the start state's post-condition against the initial
  fields before init is offered. A miss MUST fail creation and MUST NOT
  offer init.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-024`). A miss leaves fields,
  machine state, completion, and recorded sends and constructions uncommitted.
  A match commits the goto, the completion, and the constructed child's init.
  Creation fails before init when the initial fields miss the start state's
  post-condition.
