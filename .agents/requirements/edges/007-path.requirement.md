---
id: edges-007
title: "kith:path is a pure slash path"
spec_ref: .agents/specifications/edges.spec.md#path
---

# Path

## Requirement

Preconditions:

- A program imports `join`, `dirname`, `basename`, `extname`, and `resolve`
  from `kith:path`.

Expected behavior:

- The separator MUST be `/`. `(join "a" "" "b")` MUST be `a/b`. An absolute
  segment MUST restart the join.
- `(dirname "a")` MUST be `.`. `(dirname "/")` MUST be `/`.
  `(basename "/")` MUST be `""`.
- `(extname ".gitignore")` MUST be `""`. `(extname "a.tar.gz")` MUST be
  `.gz`.
- `(resolve base path)` MUST return `path` when it is absolute and otherwise
  join it onto `base`. It MUST NOT normalize `..` and MUST NOT read the
  operating system.

Postconditions:

- Tests: `src/builtin/path.rs` and `tests/edges.rs` (`req:edges-007`).
