---
id: scenes-003
title: Filling a port with the wrong kind fails check
spec_ref: .agents/specifications/scenes.spec.md#constraints
---

# Port kind fill

## Requirement

Preconditions:

- A parent declares `A a` and a scene fills that port with `a = B()`.

Expected behavior:

- Check MUST fail. The fill MUST match the port's declared kind.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:scenes-003`).
