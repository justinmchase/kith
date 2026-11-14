---
id: uffda-host-003
title: A Then pattern matches an array of values as iterable Uffda input
spec_ref: .agents/specifications/components.spec.md#matching; .agents/specifications/runtime.spec.md#reuse
---

# Match Then sequences

## Requirement

Preconditions:

- `match_message` is Uffda pattern matching. A Then of a quoted `"Check"`
  and three captures `a`, `b`, `c` each equal to `200` is the handler
  pattern.

Expected behavior:

- Matching `["Check", 200, 200, 200]` MUST succeed and bind `a`, `b`, and
  `c` to `200`.
- Matching `["Check", 200, 200]` MUST fail (wrong length).
- Matching `{ "kind": "Check", "a": 200, "b": 200, "c": 200 }` MUST fail
  (an object is not the sequence).

Postconditions:

- Tests: `tests/host.rs` (`req:uffda-host-003`).
