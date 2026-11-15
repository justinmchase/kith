---
id: edges-009
title: hello-world reads PORT and answers Hello, World!
spec_ref: .agents/specifications/edges.spec.md#hello-world; .agents/specifications/components.spec.md#sending
---

# Hello world server

## Requirement

Preconditions:

- `examples/hello-world.kith` imports `Server` from `kith:http` and
  `Process` from `kith:process`.
- The environment variable `PORT` is the listen port. The host is
  `127.0.0.1`.

Expected behavior:

- On init the server MUST send `"Env" "PORT"`. It MUST NOT listen on a
  literal port.
- `"Env" "PORT" value` MUST send `"Listen" "127.0.0.1"` with the number
  in `value`.
- `"Missing" "PORT"` MUST send `"Exit" 1`.
- On `"Listening"` it MUST print
  `Server running at http://127.0.0.1:{port}/` and `goto Listening`
  before it handles a request.
- Every `"Request"` in `Listening` MUST be answered with status 200,
  header `Content-Type` `text/plain`, and body `Hello, World!\n`.

Postconditions:

- Tests: `tests/edges.rs` (`req:edges-009`).
