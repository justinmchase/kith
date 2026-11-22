---
id: components-021
title: A filtered send down matches the parent's child record and fails when nothing matches
spec_ref: .agents/specifications/components.spec.md#routing; .agents/specifications/components.spec.md#sending; .agents/specifications/components.spec.md#syntax-tree; .agents/specifications/glossary.spec.md#components-and-trees
---

# Filtered send down

## Requirement

Preconditions:

- A parent holds a direct child, scene-constructed or dynamic. For each
  direct child the parent already holds a record `{ kind, port, key }`.
  `kind` is the component kind. `port` is the port name, or null when the
  child is unnamed. `key` is null for every scene-constructed child. A
  dynamic child's `port` is null, and its `key` is the value the
  constructing transition supplied, or null.
- The scene instance node's `key` field is still the port name of a port
  fill, or null. That field is the record's `port` for a port child, not a
  non-null `key`.

Expected behavior:

- A filtered `send down` is a third form. It MUST NOT change `send down Kind`
  or `send down any`. Kind-only and `any` omit `filter` on `sendDown`. A
  filtered send records `filter: Pattern` and sets `component` to null.
- The pattern MUST match that record and MUST NOT see the child's fields,
  machine state, or mailbox. The parent MUST NOT receive the children as a
  value.
- Each match MUST get its own send. The child MUST match that send with
  `on down`. Zero matches MUST fail the send with a diagnostic.
- An unmatched handler at a selected child MUST fail that send and MUST NOT
  forward it to the child's children.
- The source spelling is open. This form MUST NOT decide `send up P V`.

Postconditions:

- Tests: none. A test that stated this rule would have to implement filtered
  send in the runtime. That runtime is unchanged. The specs audit warns when
  no test cites this id and does not fail.
