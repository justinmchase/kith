---
id: edges-009
title: "hello-world listens on 127.0.0.1:3000 and answers Hello, World!"
spec_ref: .agents/specifications/edges.spec.md#hello-world; .agents/specifications/components.spec.md#sending
---

# Hello world server

## Requirement

Preconditions:

- `examples/hello-world.kith` imports `Server` from `kith:http`.
- `examples/http.kith` still defines its own `component Http` and does not
  import `kith:http`.

Expected behavior:

- On init the server MUST send `"Listen" "127.0.0.1" 3000`.
- On `"Listening"` it MUST print `Server running at http://127.0.0.1:3000/`.
- Every `"Request"` MUST be answered with status 200, header `Content-Type`
  `text/plain`, and body `Hello, World!\n`.

Postconditions:

- Tests: `tests/edges.rs` (`req:edges-009`).
