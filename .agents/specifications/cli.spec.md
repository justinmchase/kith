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

- Invalid input and runtime failures MUST produce diagnostics and a nonzero
  exit.
- `parse` and `check` MUST NOT construct component actors, send messages, or
  start workflows.
- `compile` MUST be reproducible from the same sources and grammar artifacts.
- `run` MUST execute only compiled trees: if given source, it compiles first
  (in memory or to the artifact path) and then interprets.
- `run` MUST start the unique exported scene of the program. `parse` prints
  one file's syntax tree as a Uffda data codec record and MUST NOT resolve
  imports. `check` and `compile` MUST resolve relative `.kith` imports before
  checking. `compile` MUST write a `.kith.ast` data codec artifact. `run` MUST
  print a data codec record of the root component's fields, or of the
  workflow succeed output when the root is a workflow. `--message` remains
  JSON for this slice.

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
