---
id: workflows-001
title: Workflows are a distinct primitive; the scene root must be a workflow
spec_ref: .agents/specifications/workflows.spec.md#distinct-primitive; .agents/specifications/scenes.spec.md#constraints
---

# Distinct primitive and scene root

## Requirement

Preconditions:

- A program declares components, workflows, and an exported scene.

Expected behavior:

- Check MUST fail if the exported scene's root kind is a component.
- Check MUST fail if a component uses `fanout`, `join`, `goto`, or `step`.
- A workflow MUST declare exactly one start step.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:workflows-001`).
