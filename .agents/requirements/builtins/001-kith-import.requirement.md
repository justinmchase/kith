---
id: builtins-001
title: kith: imports bind a registered declaration and are not fetched
spec_ref: .agents/specifications/builtins.spec.md#registration; .agents/specifications/scenes.spec.md#import-and-export
---

# kith: import

## Requirement

Preconditions:

- The host has registered `kith:data`, `kith:stream`, and `kith:time`.
- A program imports one of those specifiers, a malformed `kith:` specifier,
  or `kith:net`.

Expected behavior:

- A `kith:` URL MUST be the module URL registered with the Uffda resolver.
  Loading it MUST NOT contact a registry, MUST NOT write a lockfile, and
  MUST return the same cached declaration on the second load. An import map
  MUST NOT rewrite it. `load_path` for that URL MUST fail.
- `kith:data`, `kith:stream`, and `kith:time` MUST bind the named function,
  component, or rule. A missing name MUST fail. A name that collides with a
  local component, workflow, scene, function, or rule MUST fail.
- `kith:Data`, `kith:`, and `kith:fs/promises` MUST fail as an invalid kith
  import specifier. `kith:net` MUST fail as an unknown built-in module.
  A `jsr:` specifier MUST still fail as an unsupported import specifier.
- `kith parse` MUST NOT load the module. A relative re-export of `Pipe`,
  `bytes`, or `Bytes` MUST stay that kind in the importing file.
- No new globals. `rand` and `send after` stay as they are.

Postconditions:

- Tests: `src/builtin/mod.rs` and `tests/builtins.rs` (`req:builtins-001`).
