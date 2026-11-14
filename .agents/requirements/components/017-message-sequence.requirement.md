---
id: components-017
title: Send arguments are a value or a sequence; on Then matches that payload
spec_ref: .agents/specifications/components.spec.md#sending; .agents/specifications/components.spec.md#matching; .agents/specifications/components.spec.md#declaration
---

# Message sequences

## Requirement

Preconditions:

- A component may `send`, `complete`, or `print` one or more values after
  the route. A handler `on` lists Uffda prefixes up to the body `{`.

Expected behavior:

- One send argument MUST be that value. `send self "Tick"` MUST send
  `"Tick"`. `on self "Tick"` MUST match it.
- Two or more arguments MUST be the array of those values, in order.
  `send self "Check" s1 s2 s3` MUST send `["Check", s1, s2, s3]`.
  `on self "Check" a:200 b:200 c:200` MUST match only when the three
  captures are `200`.
- An object argument MUST remain one value. `send up { kind: "Done" status body }`
  MUST send an object. `on v:{ kind: "Done" status body }` MUST bind `v`
  to that object. `on { kind: "GET" url: any }` MUST match an object
  without commas between entries.
- A Then of N prefixes MUST NOT match an array of a different length.
- `complete` and `print` MUST take the same value list as `send`.

Postconditions:

- Tests: `tests/grammar.rs`, `tests/host.rs`, and `tests/interpreter.rs`
  (`req:components-017`).
