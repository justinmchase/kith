---
id: runtime-006
title: kith-runtime publishes to crates.io; the kith CLI package does not
spec_ref: .agents/specifications/runtime.spec.md#packages
---

# Library and CLI packages

## Requirement

Preconditions:

- The repository is a Cargo workspace with a CLI package named `kith` and a
  library package named `kith-runtime`.

Expected behavior:

- The `kith` package MUST set `publish = false`.
- The `kith-runtime` package MUST be publishable (`publish = true`).
- A language MUST depend on `kith-runtime` for check and run of a `Program`.
  It MUST NOT need the `kith` CLI package on crates.io.

Postconditions:

- Tests: `tests/packages.rs` (`req:runtime-006`).
