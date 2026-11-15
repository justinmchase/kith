---
id: edges-002
title: "kith:http speaks HTTP/1.1"
spec_ref: .agents/specifications/edges.spec.md#http
---

# Http

## Requirement

Preconditions:

- A program imports `Http` and `Server` from `kith:http`.

Expected behavior:

- `@GET url` MUST later reply `@Status status headers`. `@Pull` MUST then
  yield `@Chunk bytes` and `@End`. `@GET url headers` MUST accept an
  object of string to string or null.
- `@POST url headers` MUST reply `@Ready`. `@Push bytes` MUST reply
  `@Pushed`. `@BodyEnd` MUST reply `@Status` and MUST NOT be treated as
  `@Close`. A method other than GET or POST MUST reply `@Error`.
- A string `"Listen"` MUST reply `@Error` and MUST NOT bind.
- `@Listen port` MUST reply `@Listening "127.0.0.1" port` with the bound
  port and MUST bind `127.0.0.1`. `@Listen host port` MUST bind that host
  and reply `@Listening host port` for that listen. A second `@Listen`
  MUST reply with that listen's host and port. A request MUST be
  `@Request id method path headers`
  with the raw path. The first `@Pull id` of a GET MUST be `@End id`.
  `@Push id text` MUST append the UTF-8 bytes of `text`. `@BodyEnd id`
  MUST reply `@Responded id`.
- `https:` MUST be TLS inside this driver. The host MUST NOT follow a
  redirect.

Postconditions:

- Tests: `tests/edges.rs` (`req:edges-002`).
