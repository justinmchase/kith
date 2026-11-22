---
id: components-026
title: A field may stay in scope only in the states it names
spec_ref: .agents/specifications/components.spec.md#state-scoped-fields; .agents/specifications/components.spec.md#definitions; .agents/specifications/components.spec.md#declaration; .agents/specifications/components.spec.md#syntax-tree; .agents/specifications/glossary.spec.md#machines
---

# State-scoped fields

## Requirement

Preconditions:

- A field with `states` omitted on the field node is in scope in every
  state. That tree stays valid. Its writes stay init and `name -> E`.
- A field may instead list the states it belongs to. The list names at least
  one state of this component. The source spelling is open.

Expected behavior:

- It is one field. Its value MUST persist across `goto` among those states.
  A transition that lands outside the set MUST drop it from machine state.
  The dropped value is gone, not stored beside the machine. Entering the set
  again MUST initialize it with the init expression, or null when init is
  omitted, then the field pattern. A miss MUST fail that initialization the
  way a missed field write fails, and the transition MUST NOT commit.
- A handler in a state outside the set MUST NOT read or write the field.
  Check MUST fail. A component-scoped field and a state-scoped field MUST
  NOT share a name.
- Entry and post-condition patterns MUST name only fields in scope in that
  state. `complete` does not leave the state, so scoped fields stay until a
  later `goto` leaves the set or the instance is destroyed.
- Entering and leaving MUST be part of the proposed machine state. They MUST
  NOT run another handler and MUST NOT send.
- Inside one transition the order is: the message match selects the handler;
  assigns update component-scoped fields and any state-scoped field that
  stays in scope; new in-scope fields initialize; fields that leave the set
  are absent; the entry pattern matches, then the post-condition; the turn
  commits and routes only if both match.
- Machine state is the state name plus the fields in scope.

Postconditions:

- Tests: none. A test that stated this rule would have to implement
  state-scoped fields in the runtime. That runtime is unchanged. The specs
  audit warns when no test cites this id and does not fail.
