---
id: scenes-001
title: A scene constructs a nested instance tree from kinds, ports, and unnamed children
spec_ref: .agents/specifications/scenes.spec.md#declaration; .agents/specifications/scenes.spec.md#constraints
---

# Nested construction

## Requirement

Preconditions:

- Source declares `export scene Main { Ping() { child = Child(); } }` and
  component `Ping` with port `Child child`.

Expected behavior:

- Starting the program MUST create a `Ping` root with a `Child` filling
  `child`.
- Check MUST reject a duplicate port fill under one parent.
- Check MUST reject a construction that names an unknown kind.
- Check MUST reject a missing port fill.
- Check MUST reject `a = B()` when the parent declares port `A a`.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:scenes-001`).
