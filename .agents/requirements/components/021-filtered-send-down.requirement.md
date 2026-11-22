---
id: components-021
title: A filtered send down matches a pattern the parent holds and fails when nothing matches
spec_ref: .agents/specifications/components.spec.md#routing; .agents/specifications/components.spec.md#sending; .agents/specifications/components.spec.md#syntax-tree; .agents/specifications/glossary.spec.md#components-and-trees
---

# Filtered send down

## Requirement

Preconditions:

- A parent holds direct children. For each one the parent already holds the
  port name, if the child fills a port, and the key. `port` is that name, or
  null when the child is unnamed. `key` is null for every scene-constructed
  child.
- The scene instance node's `key` field is still the port name of a port
  fill, or null. That field is `port` on the filter record, not a non-null
  key.
- There is no `kind` field used as an is-a check. A component name in the
  pattern is a looks-like match.

Expected behavior:

- A filtered `send down` matches a Uffda pattern against that record. It
  MUST NOT require the child's declared kind to equal a named component. A
  component name in the pattern (`kind`) is a looks-like match: another
  component, including a mock, matches when it satisfies the pattern.
- `send down Pattern V` and `send down any V` keep their selector meaning.
  `any` matches every direct child. Those forms omit `filter` on `sendDown`.
  A filtered send records `filter: Pattern` and sets `component` to null.
  The message after the selector is required.
- The pattern MUST NOT see the child's fields, machine state, or mailbox.
  The parent MUST NOT receive the children as a value.
- Each match MUST get its own send. The child MUST match that send with
  `on down`. Zero matches MUST fail the send with a diagnostic.
- An unmatched handler at a selected child MUST fail that send and MUST NOT
  forward it to the child's children.
- The source spelling is open. This form MUST NOT decide `send up P V`.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:components-021`). The filter matches
  `port`, `key`, and a looks-like component name. It does not see a child's
  fields. Zero matches fail the send. An unmatched child does not forward
  the message.
