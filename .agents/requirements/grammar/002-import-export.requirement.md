---
id: grammar-002
title: Import and export lists match Uffda; relative kith files resolve
spec_ref: .agents/specifications/scenes.spec.md#import-and-export; .agents/specifications/components.spec.md#syntax-tree
---

# Import and export

## Requirement

Preconditions:

- One `.kith` file exports a component. Another file imports that name with a
  relative specifier and exports a scene that constructs it.

Expected behavior:

- `export Name Name*;` and `export` on a declaration MUST appear in the
  program `exports` list.
- `import "specifier" Name Name*;` MUST appear as `{ kind: "import",
  moduleUrl, names }`.
- `kith check` and `kith run` MUST load the relative `.kith` file and take
  the named export. A name that the other file did not export MUST be
  rejected.
- `kith parse` MUST print the importing file's tree and MUST NOT load the
  imported file.

Postconditions:

- Tests: `tests/grammar.rs` and `tests/cli.rs` (`req:grammar-002`).
