---
id: workflows-002
title: then and pick transition to the next step with the working value
spec_ref: .agents/specifications/workflows.spec.md#declaration
---

# Flow transitions

## Requirement

Preconditions:

- A workflow has `start pass ... then Next` and a later named step, or a
  `pick` with a matching clause.

Expected behavior:

- Completing a step MUST enter the `then` target (or the matching pick
  clause) with that step's output as the working value.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:workflows-002`).
