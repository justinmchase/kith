---
id: components-025
title: A state entry pattern must match the fields a transition would commit
spec_ref: .agents/specifications/components.spec.md#state-conditions; .agents/specifications/components.spec.md#identity-and-lifetime
---

# State entry pattern

## Requirement

Preconditions:

- A state may declare an entry pattern over the fields in scope in that
  state. The source spelling is open. A state that does not declare one
  passes this check.
- Enablement stays the message match in the current state. The entry pattern
  is not a guard on `on`.

Expected behavior:

- Creating an instance MUST match the start state's entry pattern against the
  initial field values before init is offered, and before the start state's
  post-condition. A miss MUST fail creation with a diagnostic and MUST NOT
  offer init.
- A transition MUST match the destination state's entry pattern against the
  fields the transition would commit, before the post-condition and before
  the commit. The destination is the state `goto` names, or the same state
  if there is no `goto`.
- The pattern MUST match. A miss is the same failure as a missed
  post-condition: a diagnostic, machine state unchanged, nothing routed.
- The entry pattern MUST NOT choose the handler. Each assign MUST still match
  that field's own pattern before the entry pattern is tried.
- The pattern MUST NOT send, construct, release, or read a clock.

Postconditions:

- Tests: none. A test that stated this rule would have to implement state
  conditions in the runtime. That runtime is unchanged. The specs audit
  warns when no test cites this id and does not fail.
