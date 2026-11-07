---
id: workflows-004
title: A task may run a child workflow until it succeeds
spec_ref: .agents/specifications/workflows.spec.md#declaration
---

# Child workflow

## Requirement

Preconditions:

- A parent workflow's start task names another workflow. The child succeeds
  with a payload.

Expected behavior:

- The parent MUST take that payload as the task output and MAY `then succeed`
  with it.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:workflows-004`).
