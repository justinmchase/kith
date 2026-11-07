# Kith requirements

Testable requirements derived from the chapters in
[`../specifications/`](../specifications/README.md), one per file under a topic
folder. Each cites the section it refines in `spec_ref`, and the tests that
verify it cite it with `req:{id}`.

## Components

- [Has-a children](./components/001-has-a-children.requirement.md)
- [One start state](./components/002-start-state.requirement.md)
- [Explicit routing](./components/003-explicit-routing.requirement.md)
- [Unmatched send up and send down](./components/004-unmatched-routing.requirement.md)
- [First matching handler](./components/005-first-match.requirement.md)
- [Ping example](./components/006-ping-example.requirement.md)

## Scenes

- [Nested construction](./scenes/001-nested-construction.requirement.md)
- [Exported scene is run](./scenes/002-exported-scene-is-run.requirement.md)
- [Port kind fill](./scenes/003-port-kind.requirement.md)

## Dispatch

- [Serial FIFO turns](./dispatch/001-serial-fifo.requirement.md)
- [Self-send later turn](./dispatch/002-self-send-later.requirement.md)

## Uffda host

- [Evaluate expressions](./uffda-host/001-evaluate-add.requirement.md)
- [Match patterns](./uffda-host/002-match-object.requirement.md)

## Grammar

- [Parse to syntax tree](./grammar/001-parse-ast.requirement.md)
- [Import and export](./grammar/002-import-export.requirement.md)

## CLI

- [Parse check compile run](./cli/001-commands.requirement.md)
