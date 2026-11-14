---
id: runtime-003
title: HealthCheck prints Healthy, Unhealthy, or Indeterminate; next Tick is armed after Validating
spec_ref: .agents/specifications/runtime.spec.md#host-io; .agents/specifications/timers.spec.md#mechanism
---

# Health check window

## Requirement

Preconditions:

- `examples/http.kith` HealthCheck owns Client and fields `s1`, `s2`,
  `s3` typed `number?`.

Expected behavior:

- Handling `"Tick"` MUST `send to client` a Go and
  `goto Waiting`. It MUST NOT `send after` Tick.
- After each Done, HealthCheck MUST shift the last three statuses,
  `send self "Check" s1 s2 s3`, then
  `goto Validating`.
- Validating MUST `(writeLine stdout "Healthy")` when `a`, `b`, and `c`
  are 200, `(writeLine stdout "Unhealthy")` when each is a number and
  none are 200, and `(writeLine stdout "Indeterminate")` otherwise. Each
  arm MUST `send after 5 self "Tick"` then `goto Checking`.
- After one or two numeric statuses it MUST write `"Indeterminate"`.
- After three 200 responses it MUST write `"Healthy"`.
- After three statuses that are each not 200 it MUST write `"Unhealthy"`.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:runtime-003`).
