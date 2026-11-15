---
id: edges-009
title: hello-world loads HOST_NAME and PORT together, then answers Hello, World!
spec_ref: .agents/specifications/edges.spec.md#hello-world; .agents/specifications/components.spec.md#sending
---

# Hello world server

## Requirement

Preconditions:

- `examples/hello-world.kith` imports `Server` from `kith:http` and
  `Process` and `OptionalValue` from `kith:process`.
- `HOST_NAME` and `PORT` are read together with one `"Env"`. The reply
  handler is `on up "Env" { HOST_NAME: OptionalValue PORT: OptionalValue }`
  and does not name the object. A null `HOST_NAME` is `127.0.0.1`. A null
  `PORT` is `3000`.

Expected behavior:

- On init the server MUST send one `"Env"` whose names are `HOST_NAME`
  and `PORT`. It MUST NOT `"Listen"` in that turn.
- The `"Env" values` reply MUST load both names before `"Listen"`. A null
  name MUST NOT `"Exit"`.
- It MUST NOT store the host or the port in a field. On
  `"Listening" host port` it MUST print
  `Server running at http://{host}:{port}/` from that message and
  `goto Listening` before it handles a request.
- Every `"Request"` in `Listening` MUST be answered with status 200,
  header `Content-Type` `text/plain`, and body `Hello, World!\n`.

Postconditions:

- Tests: `tests/edges.rs` (`req:edges-009`).
