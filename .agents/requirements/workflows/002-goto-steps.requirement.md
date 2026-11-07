---
id: workflows-002
title: goto enters another workflow step and runs its enter statements
spec_ref: .agents/specifications/workflows.spec.md#declaration
---

# Goto steps

## Requirement

Preconditions:

- A workflow has a start step that `goto`s a later step whose enter body
  assigns a field.

Expected behavior:

- After the enabling message, the instance MUST be in the later step and the
  enter assignment MUST be visible.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:workflows-002`).
