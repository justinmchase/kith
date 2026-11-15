---
id: edges-010
title: "http example imports kith:http and serves on HOST_NAME and PORT"
spec_ref: .agents/specifications/edges.spec.md#http-example
---

# Http example server

## Requirement

Preconditions:

- `examples/http.kith` imports `Server` from `kith:http` and `Process`
  from `kith:process`.
- It does not declare `component Http`.
- `HOST_NAME` and `PORT` are read with `"Env"`. A missing `HOST_NAME` is
  `127.0.0.1`. A missing `PORT` is `3000`.

Expected behavior:

- On init it MUST send `"Env" "HOST_NAME"` and MUST NOT `"Listen"`.
- After the host reply it MUST send `"Env" "PORT"`. `"Listen" host port`
  MUST wait until both values are loaded. A missing name MUST NOT `"Exit"`.
- `"Listening"` MUST `goto Listening` before a request is handled.
- Every `"Request"` in that state MUST be answered with status 200,
  header `Content-Type` `text/plain`, and body `ok\n`.

Postconditions:

- Tests: `tests/edges.rs` (`req:edges-010`).
