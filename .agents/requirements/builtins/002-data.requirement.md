---
id: builtins-002
title: kith:data bytes, blobs, and buffers are pure values
spec_ref: .agents/specifications/builtins.spec.md#data
---

# Data values

## Requirement

Preconditions:

- A program imports the `kith:data` functions and rules it calls.

Expected behavior:

- `(bytes octets)` MUST return a byte value. Empty input is empty. An octet
  outside `0`–`255`, a non-integer, or a non-array MUST fail.
- `(byte_at bytes index)` MUST return that octet and MUST fail out of range.
  `(byte_length bytes)` MUST return the octet count.
- `(bytes_concat a b …)` MUST require at least two byte values and append in
  order. `(bytes_slice bytes start end)` MUST be half-open and MUST fail
  when the range is out of bounds or `start > end`.
- `(blob type bytes)` MUST store `type` unchanged. `(blob_bytes blob)` and
  `(blob_type blob)` MUST read it back. The `Blob` rule MUST match that
  object.
- `(buffer bytes)` MUST cover the whole value. `(buffer_slice buffer start
  end)` MUST be relative to the window and MUST fail outside it.
  `(buffer_bytes buffer)` MUST return new bytes for the window. The `Buffer`
  rule MUST match a window whose bounds sit inside the bytes.
- A function that was not imported MUST fail as an unknown callee.
  `(to_codec (bytes …))` MUST emit `@bytes`, and `(from_codec …)` of that
  text MUST round-trip through `(byte_at …)`. `(sha256 text)` MUST be a
  byte value `(byte_length …)` can measure. A nested `(byte_length …)`
  inside `(add …)` MUST run.

Postconditions:

- Tests: `tests/builtins.rs` (`req:builtins-002`).
