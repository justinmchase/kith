# CLI

This chapter defines the `kith` command-line interface: the operator surface
for parsing, checking, compiling, and running Kith programs. How those commands
use Uffda is defined in [runtime](./runtime.spec.md). Terms are defined in the
[glossary](./glossary.spec.md).

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be interpreted
as described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Purpose

`kith` is a CLI in the same family as `uffda`: a distributable binary that
parses a language, emits artifacts, and runs them. It is not a wrapper that
invokes the `uffda` binary for `.kith` files. It links `uffda-runtime` and
embeds Kith's compiled grammar.

## Commands

The executable name MUST be `kith`. The initial surface MUST be:

| Command   | Does |
| --------- | ---- |
| `parse`   | Print the syntax tree of a `.kith` source file. |
| `check`   | Resolve and check a program without constructing actors or running turns. |
| `compile` | Write a versioned program artifact and print its path. |
| `run`     | Execute a source file or a compiled artifact. |
| `--version` | Print the CLI package version and exit 0. |

- Invalid input and runtime failures MUST produce diagnostics and a nonzero
  exit.
- `parse` and `check` MUST NOT construct component actors or send messages.
- `compile` MUST be reproducible from the same sources and grammar artifacts.
- `run` MUST execute only compiled trees: if given source, it compiles first
  (in memory or to the artifact path) and then interprets.
- `run` MUST start the unique exported scene of the program, offer
  construction init, then drain. `--message` is optional: a root that
  `send self`s from `on init` begins without an inject. `parse` prints
  one file's syntax tree as a Uffda data codec record and MUST NOT resolve
  imports. `check` and `compile` MUST resolve relative `.kith` imports before
  checking. `compile` MUST write a `.kith.ast` data codec artifact. `run` MUST
  print a data codec record of the root component's fields, or of its
  completion when the root has `complete`d. Writes to the
  program's `stdout` stream (`(writeLine stdout V)`, `(print V)`) MUST go
  to process stderr as data codec records so process stdout stays the run
  result. That is a stream binding, not a keyword. Constructing `Stdout`
  from `kith:stdio` takes over process stdout: pushed bytes are the process
  stdout, and `kith run` MUST NOT also write the result record. `print`
  stays on process stderr. `@Exit code` on `kith:process` sets the process
  status after the turn and MUST NOT print a reply. Arguments after the
  program path, and after `kith run`'s own flags, are the frozen argv.
  `--message` is JSON when the text is JSON: an object, a string, or an
  array of send arguments. When the text is not JSON, it is a data codec
  record, so a symbol argument is `s"Name"`.
  `kith run examples/ping.kith --message '[s"Ping" 7]'` injects the symbol
  `Ping` and `7`. `run` MUST drain mailboxes and
  advance pending `send after` timers ([timers](./timers.spec.md)).

## Distribution

The executable is a GitHub Release asset, the same shape as the `uffda`
CLI. It is not the crates.io package a higher-level language depends on.
That library is `kith-runtime`, defined in [runtime](./runtime.spec.md#packages).

- A release tag is the SemVer in the CLI package version, with no leading
  `v`.
- Each release MUST attach `kith-{version}-{target}` for the host targets
  below, `SHA256SUMS`, and `install.sh`. Windows assets use a `.exe` suffix.
- Targets: `x86_64-unknown-linux-gnu`, `aarch64-unknown-linux-gnu`,
  `x86_64-pc-windows-msvc`, `aarch64-pc-windows-msvc`,
  `x86_64-apple-darwin`, `aarch64-apple-darwin`.
- `install.sh` installs the Linux asset for the host architecture after
  checking `SHA256SUMS`.
- The grammar embedded in that binary MUST have been compiled by a released
  `uffda` CLI during the release build.

## Deferred commands

These MAY be added later. They are not required of the first executable slice.

- `fmt`
- `lsp`
- `mcp`

The grammar and source locations MUST stay suitable for formatting and a
language server.

## Project file

- A Kith application project MUST be able to name itself without an
  `uffda.jsonc` at its root. The intended names are `kith.jsonc` and
  `kith.lock`, with Uffda's project shape (`imports`, `exports`, `languages`,
  `outDir`).
- The grammar crate inside this repository MAY use `uffda.jsonc` so the
  released `uffda` CLI can compile `.uff` sources during the `kith` build.
- `--config <path>` MUST select a project file regardless of its name.

## Open questions

- **Selecting among several exported scenes** from the CLI, if a later slice
  allows more than one.
- **Exit codes** aligned with Uffda (usage/execution 2, bad project 3) vs a
  Kith-specific table.
- **Stdin / `-e` / `--json`.** Parse, compile, and run already use the data
  codec. JSON output flags MAY follow Uffda later.
