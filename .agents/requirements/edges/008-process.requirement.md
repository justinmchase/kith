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
- `OptionalValue` and `RequiredValue` are rules exported by `kith:process`.

Expected behavior:

- A string `"Argv"` MUST reply `@Error` and MUST NOT return the arguments.
- `@Argv` MUST reply `@Argv args` with the arguments frozen at start,
  after `kith run`'s own flags. A second `@Argv` MUST be the same array.
- `@Env names` MUST reply `@Env values` in one message. `names` is an
  array of one or more strings. `values` is an object: each requested name
  is its frozen string, or null when the map has no such name. A repeated
  name appears once. It MUST NOT reply once per name and MUST NOT re-read
  the operating system.
- An empty array, or a call that is not one array of strings, MUST reply
  `@Error "name"`.
- `OptionalValue` MUST match a string or null and MUST NOT match a number.
  `RequiredValue` MUST match a string and MUST NOT match null. Both stay
  exported rules.
- `@Exit code` MUST set the process status and MUST NOT reply. The run
  MUST end with that status.
- Snapshot MUST restore the frozen argv and env.

Postconditions:

- Tests: `tests/edges.rs` (`req:edges-008`).
