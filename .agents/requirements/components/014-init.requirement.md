---
id: components-014
title: Construction offers init; only on init matches; unmatched init is dropped
spec_ref: .agents/specifications/components.spec.md#identity-and-lifetime; .agents/specifications/components.spec.md#matching
---

# Construction init

## Requirement

Preconditions:

- A scene-constructed component instance has been created. HealthCheck in
  `examples/http.kith` declares `on init { send self "Tick"; }`.
  Http and Client do not declare `on init`.

Expected behavior:

- After `World::start`, before any inject, each scene-constructed component
  MUST be offered init in its start state.
- HealthCheck MUST `send self` `"Tick"` from `on init`.
- Unmatched init on Http and Client MUST be dropped with no error.
- Workflows and fan-out work items MUST NOT be offered init.
- `on down`, `on up`, `on self`, `else`, and a handler with no route MUST
  NOT match init.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-014`).
