---
id: builtins-003
title: Pipe speaks Pull, Chunk, and End
spec_ref: .agents/specifications/builtins.spec.md#stream
---

# Stream pipe

## Requirement

Preconditions:

- A program imports `Pipe`, `Chunk`, and `End` from `kith:stream` and
  constructs one `Pipe` child.

Expected behavior:

- The pipe MUST start open and empty. `"Pull"` on an open empty pipe MUST
  wait. `"Push" value` then MUST reply `"Chunk" value` and then `"Pushed"`,
  on later parent turns. The push turn MUST NOT run the puller's handler.
- A second `"Pull"` while one is waiting MUST reply `"Error" "busy"` and
  the first pull MUST stay waiting.
- `"Push"` while a value is queued MUST reply `"Error" "full"`. `"Push"` or
  `"Close"` on a closed pipe MUST reply `"Error" "closed"`.
- `"Close"` MUST reply `"Closed"`. A waiting pull also gets `"End"` before
  `"Closed"`. A queued value MUST stay: the next `"Pull"` gets
  `"Chunk" value`, and the pull after that gets `"End"`.
- An unknown command MUST reply `"Error" "unknown"` and MUST leave a
  waiting pull waiting.
- `Chunk` MUST match the tag `"Chunk"`. `End` MUST match the tag `"End"`.
  The payload MAY be any JSON value.
- The driver MUST drop init, MUST NOT run `on` handlers, and MUST NOT
  `complete`. Snapshot MUST restore open, the queue, and a waiting pull.

Postconditions:

- Tests: `src/builtin/stream.rs` and `tests/builtins.rs` (`req:builtins-003`).
