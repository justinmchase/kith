---
id: persistence-001
title: Store loads a document by key and saves it only when that version is current
spec_ref: .agents/specifications/persistence.spec.md#store
---

# Store

## Requirement

Preconditions:

- A program imports `Store` from `kith:store`.
- A parent holds one or two `Store` ports and constructs `Store()`.

Expected behavior:

- `"Save" "a" null 1` on an empty store MUST reply `"Saved" "a" 1` and
  MUST NOT fail the turn.
- `"Load" "a"` MUST then reply `"Document" "a" 1 1`. `"Load" "b"` MUST
  reply `"Missing" "b"`.
- `"Save" "a" 1 { n: 2 }` MUST reply `"Saved" "a" 2`. `"Load" "a"` MUST
  reply `"Document"` with version `2` and value `{ n: 2 }`.
- `"Save" "a" 1 { n: 9 }` after that MUST reply `"Conflict" "a" 2`, MUST
  NOT fail the turn, and the next `"Load" "a"` MUST still be version `2`
  and `{ n: 2 }`.
- `"Save" "b" 1 5` while `"b"` is absent MUST reply `"Conflict" "b" null`.
- `"Load" 1`, `"Save" "a"`, and `"Save" "a" "no" 1` MUST reply `"Error"`
  with `"key"`, `"shape"`, and `"version"`. `"Nope"` MUST reply
  `"Error" "unknown"`. None of these MUST change `"a"`.
- A second `Store` instance MUST reply `"Missing" "a"` after the first
  instance saved `"a"`.
- Snapshot after the two successful saves of `"a"` MUST contain one
  document for `"a"`, version `2`, value `{ n: 2 }`, and MUST NOT contain
  an event log. Restore MUST load that document. `Store(seed: 1)` MUST
  fail construction with a message containing `does not take construction
  arguments`.

Postconditions:

- Tests: `tests/stdlib.rs` (`req:persistence-001`).
