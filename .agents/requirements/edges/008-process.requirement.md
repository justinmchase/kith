---
id: edges-008
title: "kith:process freezes argv, env, and exit"
spec_ref: .agents/specifications/edges.spec.md#process
---

# Process

## Requirement

Preconditions:

- A program imports `Process` from `kith:process` and constructs one
  `Process`.

Expected behavior:

- `"Argv"` MUST reply `"Argv" args` with the arguments frozen at start,
  after `kith run`'s own flags. A second `"Argv"` MUST be the same array.
- `"Env" name` MUST reply `"Env" name value` when the frozen map has that
  name, and `"Missing" name` when it does not. It MUST NOT re-read the
  operating system.
- `"Exit" code` MUST set the process status and MUST NOT reply. The run
  MUST end with that status.
- Snapshot MUST restore the frozen argv and env.

Postconditions:

- Tests: `tests/edges.rs` (`req:edges-008`).
