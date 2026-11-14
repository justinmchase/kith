---
id: runtime-005
title: Async send arguments join into one message in argument order
spec_ref: .agents/specifications/runtime.spec.md#host-callables; .agents/specifications/runtime.spec.md#host-io; .agents/specifications/components.spec.md#sending
---

# Async send join

## Requirement

Preconditions:

- A handler `send`s one or more `(http_get url)` arguments and `goto`s
  a named state.

Expected behavior:

- `send self (http_get url); goto Fetching;` MUST start the get this
  turn, enter Fetching this turn, and MUST NOT block the dispatcher.
  When the host completes, Fetching MUST take `{ status: 200, url }`
  as one message.
- `send self (http_get url1) (http_get url2)` MUST deliver one array of
  two results in argument order when both complete, not two messages.
- Fetching MUST take that array as **one** message (`on a:any b:any` or
  a Then of two prefixes). Two `{ status: 200 }` object prefixes both
  bind `status`, so unique captures MUST be used. Either result not
  200 is `else` when the prefixes require it.
- `send after 1 self (http_get url)` MUST fail check.
- The mock `(http_get url)` MUST deliver `{ status: 200, url }` after
  one virtual second.
- Snapshot after `send self (http_get url)` and restore MUST still
  deliver that result.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:runtime-005`).
