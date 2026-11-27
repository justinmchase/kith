# Random

This chapter defines the numeric draws that sit beside `(rand Lo Hi)`, and
the seeded `Random` component a checker uses to fix a sequence. `(rand Lo Hi)`
and World RNG stay in [runtime](./runtime.spec.md#host-callables). Delayed
sends stay in [timers](./timers.spec.md). `Crypto` `"Random" n` stays bytes,
in [edges](./edges.spec.md#crypto). Terms are defined in the
[glossary](./glossary.spec.md).

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be interpreted
as described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Numeric draws

`(random)`, `(uniform Lo Hi)`, `(bernoulli p)`, and `(normal mean deviation)`
are same-turn host callables. They are not Uffda globals and not imports. A
call nested inside another expression MUST still run.

Each call draws on the World RNG that `(rand Lo Hi)` uses. Snapshot of that
RNG MUST cover these calls. Restore MUST continue the same stream. A call
that fails MUST NOT advance the stream. These calls MUST NOT advance the
`Crypto` byte stream, and a `Crypto` `"Random"` draw MUST NOT advance this
stream. `(id)` takes one step of this same stream for its four random
bytes, as [runtime](./runtime.spec.md#identity) defines. A failed `(id)`
MUST NOT take that step. A normal runtime seeds this generator from
entropy. A test start MAY leave it at its initial state, and two such
runs MUST agree.

| Call | Result |
| --- | --- |
| `(random)` | A number in `[0, 1)`. No arguments. An argument MUST fail. |
| `(uniform Lo Hi)` | A number in `[Lo, Hi)`. `Lo` and `Hi` are finite numbers and `Lo < Hi`. Otherwise the call MUST fail. |
| `(bernoulli p)` | `1` when the unit draw is strictly less than `p`, otherwise `0`. `p` is a finite number in `[0, 1]`. `0` MUST return `0`. `1` MUST return `1`. Otherwise the call MUST fail. |
| `(normal mean deviation)` | One finite sample from a normal distribution with that mean and deviation. The call consumes two draws. `mean` is finite. `deviation` is finite and greater than zero. Otherwise the call MUST fail. |

`(rand Lo Hi)` stays the inclusive integer range when both bounds are
integers, and the `[Lo, Hi]` draw otherwise. `(uniform Lo Hi)` does not
replace it.

## Seeded component

`import "kith:random" Random;`

`Random` is a host driver. A parent holds it by port and constructs it as
`Random(seed: n)`. `n` is a non-negative whole number. A missing seed, a
non-number, a negative, or a fraction MUST fail construction with a message
containing `seed`. A duplicate argument MUST fail with a message containing
`duplicate`. An unknown argument MUST fail with a message containing
`unknown field`. `Random` is not `Crypto`. Importing `Random` from
`kith:crypto`, or `Crypto` from `kith:random`, MUST fail resolution.

The seed is the generator state before the first draw. The same seed MUST
produce the same sequence. A different seed MUST be able to produce a
different first draw: seeds `1` and `2` MUST differ on `"Unit"`.
`Random(seed: 1)` `"Range" lo hi` MUST equal the first `(rand lo hi)` of a
new run. A draw MUST NOT advance World RNG or the `Crypto` byte stream.

The driver replies `up`. One command is one reply. A failed command MUST
NOT advance the generator.

| Down | Later up |
| --- | --- |
| `"Unit"` | `"Number" n` in `[0, 1)`. |
| `"Uniform" lo hi` | `"Number" n` in `[lo, hi)`, or `"Error" "bounds"`. |
| `"Range" lo hi` | `"Number" n` by the `(rand Lo Hi)` rule, or `"Error" "bounds"` when `lo > hi` or either value is not a number. |
| `"Bernoulli" p` | `"Number" 0` or `"Number" 1` by the `(bernoulli p)` rule, or `"Error" "probability"`. |
| `"Normal" mean deviation` | `"Number" n` by the `(normal mean deviation)` rule, or `"Error" "deviation"`. |
| anything else | `"Error" "unknown"`. |

`"Unit"`, `"Uniform"`, `"Range"`, `"Bernoulli"`, and `"Normal"` are strings.
The parent sees the reply on a later turn. `Random` MUST NOT answer
`"busy"`. Unmatched init is dropped. The driver MUST NOT run Kith `on`
handlers and MUST NOT `complete`.

Snapshot MUST include the generator state as decimal text. It MUST NOT be a
log of draws. Restore MUST answer the next draw from that state.

## Why this design

- **One World stream.** The numeric methods are the other standard draws.
  A second process RNG would split the snapshot `(rand Lo Hi)` already
  restores.
- **The seed is the checker's lever.** `Random(seed: n)` fixes the sequence
  in the scene. The checker does not script `Crypto` and does not replace
  World RNG to do it.
- **Bytes stay on Crypto.** `"Random" n` remains a byte reply. A number and
  a byte string are different answers, so they are different components.
