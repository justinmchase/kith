---
id: workflows-001
title: Workflows are a flow graph distinct from component machines
spec_ref: .agents/specifications/workflows.spec.md#constraints
---

# Distinct primitive

## Requirement

Preconditions:

- A program declares components, workflows, and an exported scene.

Expected behavior:

- Check MUST accept a scene whose root is a component.
- A workflow MUST NOT use `on` / `state` handlers.
- A workflow MUST declare exactly one `start` step.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:workflows-001`).
