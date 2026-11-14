---
id: edges-002
title: "kith:http speaks HTTP/1.1"
spec_ref: .agents/specifications/edges.spec.md#http
---

# Http

## Requirement

Preconditions:

- A program imports `Http` and `Server` from `kith:http`.
- `examples/http.kith` still defines its own `component Http` and does not
  import `kith:http`.

Expected behavior:

- `"GET" url` MUST later reply `"Status" status headers`. `"Pull"` MUST then
  yield `"Chunk" bytes` and `"End"`. `"GET" url headers` MUST accept an
  object of string to string or null.
- `"POST" url headers` MUST reply `"Ready"`. `"Push" bytes` MUST reply
  `"Pushed"`. `"BodyEnd"` MUST reply `"Status"` and MUST NOT be treated as
  `"Close"`. A method other than GET or POST MUST reply `"Error"`.
- `"Listen" port` MUST reply `"Listening" port` with the bound port. A
  request MUST be `"Request" id method path headers` with the raw path. The
  first `"Pull" id` of a GET MUST be `"End" id`. `"BodyEnd" id` MUST reply
  `"Responded" id`.
- `https:` MUST be TLS inside this driver. The host MUST NOT follow a
  redirect.

Postconditions:

- Tests: `tests/edges.rs` (`req:edges-002`).
