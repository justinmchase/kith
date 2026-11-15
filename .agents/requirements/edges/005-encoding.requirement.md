---
id: edges-005
title: "kith:encoding converts text and bytes"
spec_ref: .agents/specifications/edges.spec.md#encoding
---

# Encoding

## Requirement

Preconditions:

- A program imports the encoding functions from `kith:encoding`.

Expected behavior:

- `(utf8_encode "hi")` MUST be the octets of that UTF-8 text.
  `(utf8_decode …)` of those octets MUST return the text. Invalid UTF-8
  MUST fail the expression.
- `(base64_encode (utf8_encode "hi"))` MUST be `aGk=`. A bad decode MUST
  fail.
- `(hex_encode …)` MUST be lowercase. An odd length or a non-hex digit MUST
  fail the decode.
- The functions MUST be pure: they MUST NOT touch the filesystem, the
  network, or the process environment.

Postconditions:

- Tests: `src/builtin/encoding.rs` and `tests/edges.rs` (`req:edges-005`).
