---
id: uffda-host-001
title: Evaluation of (add 20 22) yields 42 through uffda-runtime
spec_ref: .agents/specifications/runtime.spec.md#reuse
---

# Evaluate expressions

## Requirement

Preconditions:

- The crate depends on crates.io `uffda-runtime` without `embed-languages`.

Expected behavior:

- Evaluating the Uffda expression AST for `(add 20 22)` MUST yield the number
  `42`.

Postconditions:

- Tests: `tests/host.rs` (`req:uffda-host-001`).
