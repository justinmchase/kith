---
id: edges-006
title: "kith:crypto digests bytes and draws random bytes"
spec_ref: .agents/specifications/edges.spec.md#crypto
---

# Crypto

## Requirement

Preconditions:

- A program imports `hmac_sha256`, `sha256_bytes`, and `Crypto` from
  `kith:crypto`.

Expected behavior:

- `(sha256_bytes (utf8_encode "hi"))` MUST be the SHA-256 digest
  `8f434346648f6b96df89dda901c5176b10a6d83961dd3c1ac88b59b2dc327aa4`.
- `(hmac_sha256 key data)` with key `key` and data
  `The quick brown fox jumps over the lazy dog` MUST be
  `f7bc83f430538424b13298e6aa6fb143ef4d59a14946175997479dbc2d1a3cd8`.
- `Crypto` `"Random" n` MUST reply `"Bytes" bytes` of length `n`. A
  negative or non-whole `n` MUST reply `"Error"`. The draw MUST NOT advance
  the `(rand Lo Hi)` stream. Snapshot MUST repeat the next scripted bytes.

Postconditions:

- Tests: `src/builtin/crypto.rs` and `tests/edges.rs` (`req:edges-006`).
