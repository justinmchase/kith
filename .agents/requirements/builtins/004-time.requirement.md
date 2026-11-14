---
id: builtins-004
title: kith:time is pure, and Clock is the only wall read
spec_ref: .agents/specifications/builtins.spec.md#time; .agents/specifications/timers.spec.md#definitions
---

# Time values and Clock

## Requirement

Preconditions:

- A program imports the `kith:time` names it uses.
- `send after` is still the dispatcher timer.

Expected behavior:

- `(instant text)` and `(instant_text instant)` MUST be millisecond UTC.
  `2024-01-01T00:00:00.000Z` plus `1.5` seconds MUST be
  `2024-01-01T00:00:01.5Z`. Half a millisecond MUST round away from zero.
  `(diff_seconds later earlier)` MUST be the seconds between them.
- `(zone "UTC")` MUST be utc. `(zone "+00:00")` MUST be fixed `0`.
  `(zone "+05:30")` MUST be fixed `19800`. Any other id MUST fail except
  `America/New_York`, `Europe/London`, and `Asia/Tokyo`.
- `2024-01-01T00:00:00.000Z` MUST be `2023-12-31` 19:00 in New York and
  `2024-01-01` 09:00 in Tokyo. `2024-01-01` 19:00 in New York MUST be
  `2024-01-02T00:00:00.000Z`. July in New York MUST be offset `-14400`,
  January `-18000`, Tokyo `32400`.
- `2024-03-10` 02:30 in New York MUST fail with `gap`. `2024-11-03` 01:30
  MUST fail with `overlap`. A non-Gregorian day MUST fail.
- `Clock` `"Now"` MUST reply `"Instant" instant` and MUST NOT move the
  dispatcher clock. `"Zone"` MUST reply the zone frozen at start, or the
  zone passed to `World::replace_clock`. An unknown command MUST reply
  `"Error" "unknown"`. An exhausted script MUST reply `"Error" "clock"`
  and MUST NOT record a reply.
- `World::run_until_idle` MUST NOT deliver a `send after` armed with
  `(diff_seconds later earlier)`. Snapshot MUST restore the remaining
  script so the next `"Now"` repeats the next scripted instant.

Postconditions:

- Tests: `src/builtin/time.rs` and `tests/builtins.rs` (`req:builtins-004`).
