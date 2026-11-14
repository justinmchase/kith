---
id: runtime-002
title: Http mock send-after a Response up; Client waits in Sending then send up
spec_ref: .agents/specifications/runtime.spec.md#host-io; .agents/specifications/timers.spec.md#constraints
---

# Http mock callback

## Requirement

Preconditions:

- `examples/http.kith` is loaded. The scene fills `http = Http()` under
  Client under HealthCheck.

Expected behavior:

- Construction init MUST `send self` Tick. Handling Tick MUST
  `send to client` a Go and `goto Waiting`. After `run_until_idle` Client
  MUST be in `Sending` and MUST NOT be complete. The operator MUST NOT
  inject Tick. Checking MUST NOT arm the next Tick.
- Http MUST `print` `["Requested", "https://example.com"]` and
  `send after (rand 1 3) up "Response" 200 "ok"`.
- Firing that timer MUST match Client `on up`. Client MUST `send up`
  `["Done", 200, "ok"]` and `goto Idle`.
- HealthCheck MUST match `on up` Done, `send self` Check, then
  `goto Validating`. It MUST print `"Indeterminate"` on the first window,
  `send after 5 self` Tick, then `goto Checking`. It MUST stay not
  complete. Http MUST still be in unnamed `Start`.

Postconditions:

- Tests: `tests/grammar.rs` and `tests/interpreter.rs` (`req:runtime-002`).
