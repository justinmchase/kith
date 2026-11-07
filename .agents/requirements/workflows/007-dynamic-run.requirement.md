---
id: workflows-007
title: run path executes a workflow IR from the working value
spec_ref: .agents/specifications/workflows.spec.md#dynamic-graphs
---

# Dynamic run

## Requirement

Preconditions:

- A `Host` workflow is `start run plan then succeed`. The injected value
  contains a `plan` workflow IR that tasks `Child`.

Expected behavior:

- The host MUST execute that graph and succeed with the child's complete
  payload.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:workflows-007`).
