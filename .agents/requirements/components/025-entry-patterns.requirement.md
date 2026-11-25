---
id: components-025
title: An entry pattern is matched before the post-condition
spec_ref: .agents/specifications/components.spec.md#state-conditions; .agents/specifications/components.spec.md#matching; .agents/specifications/components.spec.md#identity-and-lifetime
---

# Entry patterns

## Requirement

Preconditions:

- A state may declare `entry`, a pattern over the fields in scope. The source
  spelling stays open. Omitting `entry` means the state declares none.
- Handler choice is the route list and the message pattern. The entry
  pattern does not select a handler.
- Each assign still matches that field's own pattern before the entry
  pattern is tried.

Expected behavior:

- Creating an instance MUST match the start state's entry pattern against
  the initial fields before init is offered. A miss MUST fail creation with
  a diagnostic that says `entry pattern` and MUST NOT offer init. Creation
  does not leave a state, so it does not run the start state's
  post-condition.
- A transition MUST match the destination state's entry pattern against the
  fields after the handler's assignments, before the commit and before the
  post-condition of the state being left. `goto` uses the destination.
  `complete` does not change state, so the entry pattern of the state that
  remains is checked, and `post` does not run.
- A miss is the same failure as a missed post-condition. The diagnostic
  MUST say `entry pattern`. Machine state MUST stay as it was at the start
  of the turn. Nothing recorded is routed, including a construction.
- When the destination entry matches and the post-condition of the state
  being left does not, the diagnostic MUST say `post-condition` and name
  that state. The turn MUST NOT commit. The destination's post-condition
  does not run on the entering turn.
- A field write that misses that field's own pattern MUST fail with that
  field diagnostic before the entry pattern is tried.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-025`). A missed entry
  leaves fields, machine state, completion, and recorded sends and
  constructions uncommitted. A destination entry that matches commits the
  goto. Creation fails before init when the initial fields miss the start
  state's entry pattern.
