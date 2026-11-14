---
id: workflows-006
title: A workflow run can be snapshotted and rehydrated
spec_ref: .agents/specifications/workflows.spec.md#snapshot
---

# Snapshot and restore

## Requirement

Preconditions:

- A workflow run has been started (and MAY have in-flight mail).

Expected behavior:

- `World::snapshot` MUST produce a Uffda data codec record (not JSON).
  `World::restore` with that record MUST continue to the same succeed output
  as a run that was never snapshotted.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:workflows-006`).
