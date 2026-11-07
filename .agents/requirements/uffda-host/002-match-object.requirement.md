---
id: uffda-host-002
title: An object pattern matches a message and binds fields
spec_ref: .agents/specifications/runtime.spec.md#reuse
---

# Match patterns

## Requirement

Preconditions:

- A Uffda `over` pattern expects `kind` equal to `"Ping"` and binds `n`.

Expected behavior:

- Matching `{ "kind": "Ping", "n": 7 }` MUST succeed and bind `n` to `7`.
- Matching `{ "kind": "Pong", "n": 7 }` MUST fail.

Postconditions:

- Tests: `tests/host.rs` (`req:uffda-host-002`).
