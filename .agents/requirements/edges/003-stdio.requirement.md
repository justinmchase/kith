---
id: edges-003
title: "kith:stdio is the process byte streams"
spec_ref: .agents/specifications/edges.spec.md#stdio
---

# Stdio

## Requirement

Preconditions:

- A program imports `Stdin`, `Stdout`, and `Stderr` from `kith:stdio`.

Expected behavior:

- `Stdin` `"Pull"` MUST reply `"Chunk" bytes` or `"End"`.
- `Stdout` and `Stderr` `"Push" bytes` MUST reply `"Pushed"`. `"Close"` MUST
  reply `"Closed"`.
- Constructing `Stdout` MUST suppress the `kith run` result record. `print`
  MUST still write codec text to process stderr.
- A second `"Pull"` on `Stdin` while one is in flight MUST reply
  `"Error" "busy"`.

Postconditions:

- Tests: `tests/edges.rs` (`req:edges-003`).
