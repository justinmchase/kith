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
- [Print](./components/007-print.requirement.md)
- [Else handler](./components/008-else.requirement.md)
- [Goto state](./components/009-goto.requirement.md)
- [Unnamed start and else complete](./components/010-start-else-complete.requirement.md)
- [Mock ship callback](./components/011-mock-ship.requirement.md)
- [Route lists](./components/012-on-route.requirement.md)
- [Complete is not a state](./components/013-complete.requirement.md)
- [Construction init](./components/014-init.requirement.md)
- [Send self](./components/015-send-self.requirement.md)
- [Terminal statements](./components/016-terminal.requirement.md)
- [Message sequences](./components/017-message-sequence.requirement.md)
- [Field pattern match](./components/018-field-match.requirement.md)

## Scenes

- [Nested construction](./scenes/001-nested-construction.requirement.md)
- [Exported scene is run](./scenes/002-exported-scene-is-run.requirement.md)
- [Port kind fill](./scenes/003-port-kind.requirement.md)

## Dispatch

- [Serial FIFO turns](./dispatch/001-serial-fifo.requirement.md)
- [Self-send later turn](./dispatch/002-self-send-later.requirement.md)
- [Delayed send](./dispatch/003-send-after.requirement.md)

## Uffda host

- [Evaluate expressions](./uffda-host/001-evaluate-add.requirement.md)
- [Match patterns](./uffda-host/002-match-object.requirement.md)
- [Match Then sequences](./uffda-host/003-match-then.requirement.md)

## Runtime

- [Rand host callable](./runtime/001-rand.requirement.md)
- [Http mock callback](./runtime/002-http-mock.requirement.md)
- [Health check window](./runtime/003-health-check.requirement.md)

## Workflows

- [Distinct primitive](./workflows/001-distinct-and-root.requirement.md)
- [Then and pick](./workflows/002-goto-steps.requirement.md)
- [Map fan-out and reduce](./workflows/003-fanout-join.requirement.md)
- [Child workflow](./workflows/004-child-workflow.requirement.md)
- [Order wait, confirm, summarize](./workflows/005-ping-workflow.requirement.md)
- [Snapshot and restore](./workflows/006-snapshot-restore.requirement.md)
- [Dynamic run](./workflows/007-dynamic-run.requirement.md)
- [Step pattern and into init](./workflows/008-step-pattern-into.requirement.md)

## Grammar

- [Parse to syntax tree](./grammar/001-parse-ast.requirement.md)
- [Import and export](./grammar/002-import-export.requirement.md)

## CLI

- [Parse check compile run](./cli/001-commands.requirement.md)
