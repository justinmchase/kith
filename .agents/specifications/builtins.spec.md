# Built-ins

This chapter defines the `kith:` modules that ship in the binary: how a program
imports them, and the data, stream, and time modules. Dispatch of the messages
those components reply with is defined in [dispatch](./dispatch.spec.md).
Delayed sends stay in [timers](./timers.spec.md). Terms are defined in the
[glossary](./glossary.spec.md).

`kith:net`, `kith:http`, `kith:stdio`, `kith:fs`, `kith:encoding`,
`kith:crypto`, `kith:path`, and `kith:process` are not part of this chapter.

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be interpreted
as described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Registration

- A built-in specifier MUST be `kith:` plus one lowercase module name
  (`kith:data`). That string is the module URL. It MUST NOT carry a version,
  a slash, or a subpath. `kith:fs/promises`, `kith:Data`, and `kith:` are
  invalid specifiers.
- Before resolution the host MUST register an immutable declaration for each
  module this chapter defines with the Uffda resolver. The declaration is
  cached by that URL. Loading it MUST NOT contact a registry, MUST NOT read a
  cache directory, MUST NOT write a lockfile entry, and MUST NOT re-lower the
  declaration. A second load of the same URL MUST return the same cached
  declaration.
- An import map MUST NOT rewrite a `kith:` URL. `jsr:` specifiers and relative
  `.kith` specifiers stay as [scenes](./scenes.spec.md#import-and-export)
  defines them. `jsr:` is not required to load a Kith program.
- A specifier that has the `kith:` shape and names no registered module
  (`kith:net`) MUST fail as an unknown built-in. A specifier that contains
  `:` and is neither `kith:` nor a relative path MUST fail as an unsupported
  import specifier.
- The names after the specifier are the exports to bind. A missing export
  MUST fail resolution. A name that collides with a component, workflow,
  scene, function, or rule already in that file MUST fail resolution.
- `kith parse` MUST NOT load the imported module. `kith check` and `kith run`
  MUST.
- A relative import of a name this module re-exported MUST bind that same
  kind when the importing file does not already declare the name. A host
  component stays a host driver. A local declaration of that name still wins,
  as it does for any other relative import.

## Kinds of export

Every export of a module in this chapter is a function, a component, or a
rule. The registered declaration lists the functions and rules. Components
are Kith host drivers for that module name; they are not Uffda exports.

- A **function** is a pure host callable. It returns on the same turn, takes
  only its arguments, and MUST NOT send, wait, or read the dispatcher clock,
  the wall clock, the filesystem, the network, or the process environment.
  It is callable only when imported. Otherwise the callee is unknown.
  A call nested inside another expression MUST still run.
- A **component** is a scene-constructed child. The program `send to`s a
  request. A later message comes `up`. The driver MUST NOT run Kith `on`
  handlers and MUST NOT `complete`. Unmatched init is dropped. One instance
  has one in-flight stream command.
- A **rule** is a pattern name in scope after import. A handler or a field
  uses it as a pattern (`field got: Bytes`, `on up Chunk part:Bytes`).
  Matching a rule MUST NOT send, pull, or read a clock. Kith matches an
  imported rule. The declaration records the export; it is not a second
  definition of the rule.

No new globals. `rand`, `print`, `send after`, and the Uffda globals stay as
their chapters define them. Byte length is `(byte_length bytes)`, not
`length`.

## Data

`import "kith:data" Bytes Blob Buffer bytes byte_at byte_length bytes_concat bytes_slice blob blob_bytes blob_type buffer buffer_slice buffer_bytes;`

`Bytes` is an Uffda byte value. At a codec boundary it is `@bytes` with
canonical unpadded base64url. `Blob` and `Buffer` are plain objects. Nothing
in this module is mutable, and nothing reads a device.

| Export | Kind | Behavior |
| --- | --- | --- |
| `Bytes` | rule | Matches a byte value. |
| `Blob` | rule | Matches `{ type: string, bytes: Bytes }`. Extra keys are ignored. |
| `Buffer` | rule | Matches `{ bytes: Bytes, start: number, end: number }` with whole numbers and `0 <= start <= end <= byte length`. Extra keys are ignored. |
| `(bytes octets)` | func | `octets` is an array of integers `0`–`255`. Returns `Bytes`. `(bytes [])` is empty. A non-array, a non-integer, or an octet outside `0`–`255` MUST fail. |
| `(byte_at bytes index)` | func | The octet at `index`. An index that is not a whole number, or is outside the value, MUST fail. |
| `(byte_length bytes)` | func | The number of octets. |
| `(bytes_concat a b)` | func | New `Bytes`. At least two arguments. Further arguments append in order. |
| `(bytes_slice bytes start end)` | func | New `Bytes` for the half-open range. `start > end`, or a bound outside the value, MUST fail. |
| `(blob type bytes)` | func | A `Blob`. `type` is stored and MUST NOT be parsed as a media type. |
| `(blob_bytes blob)` | func | The blob's `Bytes`. |
| `(blob_type blob)` | func | The media-type string. |
| `(buffer bytes)` | func | A `Buffer` covering the whole value (`start` 0, `end` the length). |
| `(buffer_slice buffer start end)` | func | A `Buffer` whose range is relative to that window. A range outside the window MUST fail. |
| `(buffer_bytes buffer)` | func | New `Bytes` for the window. |

`(to_codec bytes)` MUST emit `@bytes`. `(from_codec text)` of that record
MUST be a byte value `(byte_at …)` can read. `(sha256 text)` remains the
global and MUST return a byte value.

## Stream

`import "kith:stream" Chunk End Pipe;`

`"Pull"` asks for the next value. `"Chunk" value` is that value. `"End"` is
eof. `Chunk` matches the tag `"Chunk"`. `End` matches the tag `"End"`.
`on up Chunk part:Bytes` is a chunk whose payload matches `Bytes`.

`Pipe` is an in-program stream. It starts open. At most one value is queued.
It carries any JSON value, not only bytes. The driver replies `up`:

| Down | Later up |
| --- | --- |
| `"Pull"` | `"Chunk" value` when a value is queued. `"End"` when closed and empty. No reply yet when open and empty: the pull waits. |
| `"Push" value` | `"Pushed"`. `"Error" "full"` when a value is already queued. `"Error" "closed"` when the pipe is closed. |
| `"Close"` | `"Closed"`. A waiting `"Pull"` also gets `"End"`, and that `"End"` is delivered before `"Closed"`. A queued value stays; the next `"Pull"` gets `"Chunk" value`, and the `"Pull"` after that gets `"End"`. |

While a `"Pull"` is waiting, another `"Pull"` MUST reply `"Error" "busy"`
and the first pull MUST stay waiting. A `"Push"` while a pull is waiting
answers that pull. It is not busy. The pipe's turn emits `"Chunk" value`
and then `"Pushed"`. It MUST NOT run the puller's handler; both replies are
later turns. `"Close"` while a pull is waiting emits `"End"` and then
`"Closed"`.

A command the table does not name MUST reply `"Error" "unknown"`. A waiting
pull stays waiting. A second `"Close"` when the pipe is already closed MUST
reply `"Error" "closed"`.

Snapshot MUST include, for each pipe instance, whether it is open, the
queued value if there is one, and whether a pull is waiting. Restore MUST
continue that pipe.

## Time

`import "kith:time" Instant DateTime Zone Clock instant instant_text zone date_time instant_at add_seconds diff_seconds offset_seconds;`

An instant is an Uffda Date: a millisecond UTC instant. At a codec boundary
it is `@date` in RFC3339. A zone and a civil date are plain objects. The
functions are pure. They MUST NOT read the OS clock or a zone database on
the machine.

The dispatcher **Clock** in [timers](./timers.spec.md#definitions) is not
this component. `send after` stays a number of seconds on that dispatcher
clock. A handler MUST NOT read it. `(diff_seconds later earlier)` is how a
program turns two instants into that delay. `kith:time` MUST NOT export
`sleep`, `setTimeout`, or a delay function.

| Rule | Matches |
| --- | --- |
| `Instant` | A millisecond UTC instant. |
| `DateTime` | `{ year, month, day, hour, minute, second, millisecond }`, whole numbers. Year `-9999`–`9999`. Month `1`–`12`. Day a real Gregorian day. Hour `0`–`23`. Minute `0`–`59`. Second `0`–`59` (no leap second). Millisecond `0`–`999`. |
| `Zone` | `{ kind: "utc" }`, `{ kind: "fixed", seconds: number }` (whole seconds east of UTC), or `{ kind: "iana", id: string }` for an id in the table below. |

| Func | Returns |
| --- | --- |
| `(instant text)` | The instant for RFC3339 text. Invalid text, or a precision other than milliseconds, MUST fail. |
| `(instant_text instant)` | The canonical `@date` text for that instant: RFC3339 UTC. A zero fraction is omitted (`2024-01-02T00:00:00Z`). A non-zero fraction keeps only the significant digits (`2024-01-01T00:00:01.5Z`). |
| `(zone id)` | `"UTC"` is `{ kind: "utc" }`. `+HH:MM` and `-HH:MM`, hours `00`–`23` and minutes `00`–`59`, are `{ kind: "fixed", seconds }`. `+00:00` is fixed `0`, not utc. A table id is `{ kind: "iana", id }`. Anything else MUST fail. |
| `(date_time instant zone)` | The civil `DateTime` of that instant in that zone. |
| `(instant_at date zone)` | The instant that civil time denotes. A DST gap MUST fail with a message containing `gap`. An overlap MUST fail with a message containing `overlap`. |
| `(add_seconds instant n)` | The instant `n` seconds later. `n` MAY be negative. Sub-millisecond fractions round to the nearest millisecond; a fraction of exactly half a millisecond rounds away from zero. Overflow MUST fail. |
| `(diff_seconds later earlier)` | Seconds from `earlier` to `later`, as a number. The earlier instant need not be first. |
| `(offset_seconds zone instant)` | Seconds east of UTC for that zone at that instant. |

The zone table shipped in the binary is `America/New_York`, `Europe/London`,
and `Asia/Tokyo`.

- `America/New_York` is UTC−5, and UTC−4 from the second Sunday of March at
  07:00 UTC until the first Sunday of November at 06:00 UTC.
  `2024-03-10` 02:30 is a gap. `2024-11-03` 01:30 is an overlap.
- `Europe/London` is UTC+0, and UTC+1 from the last Sunday of March at
  01:00 UTC until the last Sunday of October at 01:00 UTC. Local `01:30` on
  that March Sunday is a gap. Local `01:30` on that October Sunday is an
  overlap.
- `Asia/Tokyo` is UTC+9. It has no gap and no overlap.
- `2024-01-01T00:00:00Z` is `2023-12-31` 19:00 in New York and
  `2024-01-01` 09:00 in Tokyo. `2024-01-01` 19:00 in New York is
  `2024-01-02T00:00:00Z`. Input MAY use a zero millisecond fraction
  (`2024-01-01T00:00:00.000Z`); `(instant_text …)` emits the canonical form.

`Clock` is the only wall read. The process zone is the `TZ` environment
value at `World::start` when that value is a zone id this chapter accepts,
and UTC otherwise. It is frozen for the run. `"Zone"` MUST return that
frozen value and MUST NOT read `TZ` again. A checker replaces the driver
with `World::replace_clock(zone, script)`. `zone` is a zone id.
`script` is RFC3339 instants, one per later `"Now"`, in order.

| Down | Later up |
| --- | --- |
| `"Now"` | `"Instant" instant`. The next scripted instant, or the wall clock when no script is installed. An exhausted script MUST reply `"Error" "clock"` and MUST NOT record a reply. |
| `"Zone"` | `"Zone" zone`. |
| anything else | `"Error" "unknown"`. |

`"Now"` and `"Zone"` reply on that driver turn. The parent sees them on a
later turn. `Clock` MUST NOT answer `"busy"`. A `"Now"` MUST NOT move the
dispatcher clock, and `World::run_until_idle` MUST NOT deliver a `send after`
because a clock reply arrived.

Snapshot MUST include the frozen zone, the remaining script (`null` when the
run is reading the wall), and the `"Now"` replies already produced, in order.
Restore MUST answer the next `"Now"` with the next remaining scripted
instant. Per-instance driver state for `Pipe` is part of the same snapshot.

## Why this design

- **The prefix is not a download.** A built-in has no version range and no
  lockfile entry. Upgrading `kith` upgrades the modules. `jsr:` stays the
  scheme that fetches.
- **Import, don't global.** A wall read or a byte constructor that was always
  in scope would hide a source of variation, or a second runtime, in every
  handler. The import is the edge.
- **The driver is a child.** `Pipe` and `Clock` use the same `send to` /
  later `up` shape as the `Http` mock in `examples/http.kith`. That mock
  stays a user component. This chapter does not bind it to a socket.
- **The dispatcher clock stays virtual.** `send after` is still seconds from
  zero. The wall is a message a checker can script. Connecting them is
  `(diff_seconds later earlier)`, and the program writes that number itself.
