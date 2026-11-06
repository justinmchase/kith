---
id: grammar-001
title: Kith source parses to the component syntax tree
spec_ref: .agents/specifications/components.spec.md#syntax-tree
---

# Parse to syntax tree

## Requirement

Preconditions:

- The Kith language grammar is compiled with Uffda and embedded in the binary.

Expected behavior:

- Parsing the normative Parent/Child source MUST produce a `program` tree whose
  `root` is `Parent`, with `Child` and `Parent` components, `heard` field, and
  `child` slot.
- The tree MUST be the same shape the interpreter accepts as a hand-built AST.

Postconditions:

- Tests: `tests/grammar.rs` (`req:grammar-001`).
