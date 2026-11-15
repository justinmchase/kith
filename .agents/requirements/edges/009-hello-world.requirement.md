---
id: edges-009
title: hello-world defaults HOST_NAME and PORT, then answers Hello, World!
spec_ref: .agents/specifications/edges.spec.md#hello-world; .agents/specifications/components.spec.md#sending
---

# Hello world server

## Requirement

Preconditions:

- `examples/hello-world.kith` imports `Server` from `kith:http` and
  `Process` from `kith:process`.
- `HOST_NAME` and `PORT` are read with `"Env"`. A missing `HOST_NAME` is
  `127.0.0.1`. A missing `PORT` is `3000`.

Expected behavior:

- On init the server MUST send `"Env" "HOST_NAME"` and MUST NOT `"Listen"`.
- After the host reply it MUST send `"Env" "PORT"`. `"Listen" host port`
  MUST wait until both values are loaded. A missing name MUST NOT `"Exit"`.
- On `"Listening"` it MUST print
  `Server running at http://{host}:{port}/` and `goto Listening`
  before it handles a request.
- Every `"Request"` in `Listening` MUST be answered with status 200,
  header `Content-Type` `text/plain`, and body `Hello, World!\n`.

Postconditions:

- Tests: `tests/edges.rs` (`req:edges-009`).
