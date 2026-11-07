---
id: scenes-002
title: kith run and World start use the unique exported scene
spec_ref: .agents/specifications/scenes.spec.md#constraints; .agents/specifications/cli.spec.md#commands
---

# Exported scene is run

## Requirement

Preconditions:

- A program declares one or more scenes and an export list.

Expected behavior:

- `World::start` and `kith run` MUST construct the unique exported scene.
- A program that exports no scene MUST be rejected.
- A program that exports more than one scene MUST be rejected.

Postconditions:

- Tests: `tests/interpreter.rs` and `tests/cli.rs` (`req:scenes-002`).
