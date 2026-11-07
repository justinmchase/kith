---
id: grammar-001
title: Kith source parses to the component and scene syntax tree
spec_ref: .agents/specifications/components.spec.md#syntax-tree
---

# Parse to syntax tree

## Requirement

Preconditions:

- The Kith language grammar is compiled with Uffda and embedded in the binary.

Expected behavior:

- Parsing the normative Ping/Child source MUST produce a `program` tree with
  exports `Child`, `Ping`, and `Main`, component `Child`, workflow `Ping` with
  a `heard` field and a `Child child` port, and scene `Main` whose root is
  `Ping` with a child instance of `Child` filling port `child`.
- The tree MUST be the same shape the interpreter accepts as a hand-built AST.
- The tree MUST NOT include a program `root` string or `children` on a
  component.

Postconditions:

- Tests: `tests/grammar.rs` (`req:grammar-001`).
