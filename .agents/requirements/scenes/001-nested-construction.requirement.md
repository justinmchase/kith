---
id: scenes-001
title: A scene constructs a nested instance tree from kinds and slot keys
spec_ref: .agents/specifications/scenes.spec.md#declaration; .agents/specifications/scenes.spec.md#constraints
---

# Nested construction

## Requirement

Preconditions:

- Source declares `export scene Main { Parent() { child = Child() } }` and
  both kinds.

Expected behavior:

- Starting the program MUST create a `Parent` root and one `Child` in slot
  `child`.
- Check MUST reject a duplicate slot key under one parent.
- Check MUST reject a construction that names an unknown kind.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:scenes-001`).
