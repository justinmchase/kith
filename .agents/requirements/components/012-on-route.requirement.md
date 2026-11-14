---
id: components-012
title: on lists routes with or; unqualified on is up or down or self
spec_ref: .agents/specifications/components.spec.md#declaration; .agents/specifications/components.spec.md#matching
---

# Route-qualified handlers

## Requirement

Preconditions:

- `examples/ping.kith` declares Child `on down` Ping and Ping `on up` Pong.
- `examples/http.kith` Client is `on down` Go in Idle and `on up` Response
  in Sending.

Expected behavior:

- Injecting `["Ping", 7]` at Ping MUST match `on down` and MUST
  set `heard` to `7` after Child `send up`s Pong.
- After Client takes Go it MUST be in `Sending`. `World::send` of
  `["Response", 200, "ok"]` into Client MUST fail as unmatched down. Mock
  Http's delayed `send up` MUST match `on up`; Client MUST `send up` Done
  and `goto Idle`.
- Unqualified `on pattern` MUST project route `["up", "down", "self"]`.
- `on up or down pattern` MUST match send up and send down / inject, and
  MUST NOT match `send self`.
- `on down or down` MUST fail `check`.

Postconditions:

- Tests: `tests/grammar.rs` and `tests/interpreter.rs` (`req:components-012`).
