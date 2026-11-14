---
id: components-013
title: complete sends up, does not goto, and takes no further turns
spec_ref: .agents/specifications/components.spec.md#sending; .agents/specifications/glossary.spec.md#workflows
---

# Complete is not a state

## Requirement

Preconditions:

- A scene root component handles inject with `complete { kind: "Done" }`.

Expected behavior:

- That turn MUST mark the instance complete. `World::state` MUST still be
  the state that ran `complete` (`Start`), not a `Completed` state.
- Completing the root MUST NOT send up.
- A later inject MUST fail as unmatched and MUST NOT run another
  transition.
- `examples/http.kith` Http MUST `send after` up and MUST NOT `complete`.
  After `run_until_idle` Http MUST still be in `Start` and MUST NOT be
  complete.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-013`).
