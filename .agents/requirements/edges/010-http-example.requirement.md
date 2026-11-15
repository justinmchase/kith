---
id: edges-010
title: "http example imports kith:http and serves on PORT"
spec_ref: .agents/specifications/edges.spec.md#http-example
---

# Http example server

## Requirement

Preconditions:

- `examples/http.kith` imports `Server` from `kith:http` and `Process`
  from `kith:process`.
- It does not declare `component Http`.
- The environment variable `PORT` is the listen port. The host is
  `127.0.0.1`.

Expected behavior:

- On init it MUST send `"Env" "PORT"`. `"Env" "PORT" value` MUST send
  `"Listen" "127.0.0.1"` with the number in `value`.
- `"Listening"` MUST `goto Listening` before a request is handled.
- Every `"Request"` in that state MUST be answered with status 200,
  header `Content-Type` `text/plain`, and body `ok\n`.

Postconditions:

- Tests: `tests/edges.rs` (`req:edges-010`).
