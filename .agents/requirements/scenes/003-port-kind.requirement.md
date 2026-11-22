---
id: scenes-003
title: A port fill must satisfy the port pattern
spec_ref: .agents/specifications/scenes.spec.md#constraints
---

# Port pattern fill

## Requirement

Preconditions:

- A parent declares `a: A`. `A` is a looks-like selector, not an is-a bind
  on the declared component. The spelling stays `a: A`.

Expected behavior:

- A fill MUST satisfy that pattern. The instance does not have to be the
  declared component `A`. A mock MAY fill the port when it satisfies the
  pattern.
- Check MUST fail when the fill does not satisfy the pattern. A different
  declared kind is not, by itself, that failure.

Postconditions:

- Tests: `tests/interpreter.rs` (`req:scenes-003`) still runs the unchanged
  runtime, which rejects a different declared kind. That run does not
  establish this requirement.
