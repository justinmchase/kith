# Persistence

This chapter defines `Store`, the child that keeps the current document for
a key. How a relocated actor resumes is not decided here. Dispatch of the
replies is defined in [dispatch](./dispatch.spec.md). Importing a `kith:`
module is defined in [built-ins](./builtins.spec.md#registration). Terms are
defined in the [glossary](./glossary.spec.md).

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be interpreted
as described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Store

`import "kith:store" Store;`

`Store` is a host driver. `Store()` takes no arguments. A construction
argument MUST fail with a message containing `does not take construction
arguments`. Each instance keeps its own documents. A save on one instance
MUST NOT appear in another.

The runtime keeps the current document and its version for each key. It
MUST NOT keep an event log. A second successful save of a key replaces the
document. Snapshot of that instance MUST include one entry for the key, the
version, and the value, and MUST NOT include the replaced value.

A version is a positive whole number. The first successful save of a key
stores version `1`. Each later successful save stores the previous version
plus one.

Conflict, a successful save, and a missing key are replies. They MUST NOT
fail the turn. A reply the table does not name is `"Error"` with a reason,
and that reply MUST NOT change a document.

The caller names the version it loaded. `null` means the caller believes
the key is absent. A save whose named version is not the version currently
stored MUST reply `"Conflict"` and MUST NOT write.

| Down | Later up |
| --- | --- |
| `"Load" key` | `"Document" key version value` when the key is stored. `"Missing" key` when it is not. |
| `"Save" key null value` | `"Saved" key 1` when the key is absent. `"Conflict" key version` when a document is already stored. |
| `"Save" key version value` | `"Saved" key next` when `version` is the stored version. `next` is that version plus one. `"Conflict" key stored` when it is not. `stored` is `null` when the key is absent. |
| anything else | `"Error" "unknown"`. |

`key` is a string. An empty string is a key. A key that is not a string
MUST reply `"Error" "key"`. A `"Load"` or `"Save"` whose argument count is
wrong MUST reply `"Error" "shape"`. A `"Save"` version that is neither
`null` nor a positive whole number MUST reply `"Error" "version"`. None of
these replies write.

`"Load"`, `"Save"`, `"Document"`, `"Missing"`, `"Saved"`, `"Conflict"`, and
`"Error"` are strings. The parent sees the reply on a later turn. `Store`
MUST NOT answer `"busy"`. Unmatched init is dropped. The driver MUST NOT
run Kith `on` handlers and MUST NOT `complete`.

Snapshot MUST include each instance's current documents. Restore MUST load
and save against those documents.

## Why this design

- **The document is the state.** The caller loads a value and saves a value.
  A log of commands would make the stored state a fold the checker has to
  replay, and this chapter does not define that fold.
- **The version is the check.** Two saves that both loaded the same version
  cannot both write. The second is `"Conflict"`, which a handler can match.
  It is not an exception, so the machine stays in the state that sent the
  save.
- **One child, one map.** Coleslaw's state store is this child. A second
  child is a second map, so a checker can hold a store that nothing else
  writes.

## Open questions

- **Relocation.** What of a durable component is stored, and how a relocated
  actor resumes, is not this chapter. `Store` does not resume an actor.
