---
id: components-024
title: A state post-condition must match before a transition commits
spec_ref: .agents/specifications/components.spec.md#state-conditions; .agents/specifications/components.spec.md#behavior
---

# State post-condition

## Requirement

Preconditions:

- A state may declare a post-condition pattern over the fields in scope in
  that state. The source spelling is open. A state that does not declare one
  passes this check.
- Each field write still matches that field's own pattern, init and
  `name -> E`, before this pattern is tried.

Expected behavior:

- The post-condition is an extra pattern on the state change. It MUST NOT
  select the handler. The same machine state and the same matched message
  MUST produce the same post-condition match.
- For the proposed result (assigns applied, `goto` chosen, or the same state
  if there is no `goto`), the entry pattern is matched first, then this
  post-condition, against the fields that would be in scope. Both MUST match.
  Only then does the turn commit machine state and route what it recorded.
- A miss MUST fail the transition with a diagnostic. Machine state MUST stay
  as it was at the start of the turn. Nothing recorded is routed.
- `complete` does not change state. The post-condition of the state that
  remains MUST still be checked.
- The pattern MUST NOT send, construct, release, or read a clock. It is not
  an exit action and not a `succeed` or `fail`.
- Creation MUST check the start state's post-condition against the initial
  fields before init is offered. A miss MUST NOT offer init.

Postconditions:

- Tests: none. A test that stated this rule would have to implement state
  conditions in the runtime. That runtime is unchanged. The specs audit
  warns when no test cites this id and does not fail.
