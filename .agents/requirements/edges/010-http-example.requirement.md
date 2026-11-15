---
id: edges-010
title: "http example imports kith:http and serves on HOST_NAME and PORT"
spec_ref: .agents/specifications/edges.spec.md#http-example
---

# Http example server

## Requirement

Preconditions:

- `examples/http.kith` imports `Server` from `kith:http` and `Process`
  and `OptionalValue` from `kith:process`.
- It does not declare `component Http`.
- `HOST_NAME` and `PORT` are read together with one `"Env"`. The reply
  handler is `on up "Env" { HOST_NAME: OptionalValue PORT: OptionalValue }`
  and does not name the object. A null `HOST_NAME` is `127.0.0.1`. A null
  `PORT` is `3000`.

Expected behavior:

- On init it MUST send one `"Env"` whose names are `HOST_NAME` and
  `PORT`. It MUST NOT `"Listen"` in that turn.
- The `"Env" values` reply MUST load both names before `"Listen"`. A null
  name MUST NOT `"Exit"`.
- `"Listening"` MUST `goto Listening` before a request is handled.
- Every `"Request"` in that state MUST be answered with status 200,
  header `Content-Type` `text/plain`, and body `ok\n`.

Postconditions:

- Tests: `tests/edges.rs` (`req:edges-010`).
