---
id: workflows-004
title: A workflow port may be filled by a child workflow that completes
spec_ref: .agents/specifications/workflows.spec.md#child-workflows
---

# Child workflow

## Requirement

Preconditions:

- A parent workflow declares a port whose kind is a workflow. The scene fills
  it. The parent sends to that port; the child `complete`s.

Expected behavior:

- The parent MUST receive the completion as send up and MAY record it in a
  field.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:workflows-004`).
