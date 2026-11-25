---
id: components-026
title: A field may name the states it belongs to
spec_ref: .agents/specifications/components.spec.md#state-scoped-fields; .agents/specifications/components.spec.md#declaration; .agents/specifications/glossary.spec.md#machines
---

# State-scoped fields

## Requirement

Preconditions:

- A field with `states` omitted is in scope in every state. A field may set
  `states` to the state names it belongs to. The source spelling stays open.
  Omitting `states` means every state.
- One name is one field. A component-scoped field and a state-scoped field
  do not share a name.
- The list names at least one state of this component. An empty list or an
  unknown name fails check.

Expected behavior:

- The value persists across `goto` among the named states. A transition that
  lands outside the set drops it from machine state. The value that left is
  gone. Entering the set again initializes it with the init expression, or
  null when init is omitted, then the field pattern. A miss MUST fail that
  initialization the way a missed field write fails (`field {name} did not
  match`) and the transition MUST NOT commit. Nothing recorded is routed.
- A handler outside the set cannot read or write the field. Check MUST fail
  and say `not in scope`.
- Entry and post-condition patterns name only fields in scope in that state.
  Check MUST fail if they name a field that is not.
- `complete` does not leave the state, so a scoped field stays.
- Entering and leaving do not run another handler and do not send.
- At creation, fields in scope in the start state initialize, and
  state-scoped fields outside that set are absent, before the entry pattern.
  Creation does not leave a state, so it does not run the start state's
  post-condition.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-026`). A scoped field
  persists across the states it names, drops on the way out, and initializes
  again on the way back. A failed initialization leaves the machine
  unchanged and does not route a recorded send. `complete` keeps the field.
  Check rejects a handler, an entry pattern, a duplicate name, an empty
  list, and an unknown state.
