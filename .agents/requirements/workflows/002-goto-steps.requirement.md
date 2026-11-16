---
id: workflows-002
title: arrow and pick transition to the next step with the working value
spec_ref: .agents/specifications/workflows.spec.md#declaration
---

# Flow transitions

## Requirement

Preconditions:

- A workflow has `start begin: pass ... -> next` and a later named step, or a
  `pick` with a matching clause.

Expected behavior:

- Completing a step MUST enter the `->` target (or the matching pick
  clause) with that step's output as the working value.
- `name: Kind -> Name` MUST be that edge. `Kind then Name` and a step with
  no name before the colon MUST NOT parse. An input object on `-> Name`
  replaces the next step's input; with no object the input is the step
  output.

Postconditions:

- Tests: `tests/interpreter.rs` and `tests/grammar.rs` (`req:workflows-002`).
