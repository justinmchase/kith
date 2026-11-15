# Edges

This chapter defines the host modules that touch a device or the process:
`kith:net`, `kith:http`, `kith:stdio`, `kith:fs`, `kith:encoding`,
`kith:crypto`, `kith:path`, and `kith:process`. How a program imports a
`kith:` module is defined in [built-ins](./builtins.spec.md#registration).
Byte values and the pull protocol are defined in
[built-ins](./builtins.spec.md#data) and
[built-ins](./builtins.spec.md#stream). `send after` stays in
[timers](./timers.spec.md).

`examples/http.kith` imports `kith:http` and runs as a server. It does not
declare `component Http`. The user mock is `examples/health.kith`.

## Conventions

The key words MUST, MUST NOT, SHOULD, SHOULD NOT, and MAY are to be interpreted
as described in RFC 2119 and RFC 8174 when, and only when, they appear in all
capitals.

## Shared rules

- A function in this chapter is pure. It returns on the same turn and MUST
  NOT send, wait, or read the clock, the filesystem, the network, or the
  process environment.
- A component is a host driver. The program `send to`s a request. A later
  message comes `up`. The driver MUST NOT run Kith `on` handlers and MUST
  NOT `complete`. Unmatched init is dropped.
- One instance has one in-flight stream command, except `Listener` and
  `Server`, which are in flight per id. A second `Pull` or `Push` for that
  same in-flight command MUST reply `"Error" "busy"`, or `"Error" id "busy"`
  when the command carries an id. The first command MUST stay in flight.
- An unknown command MUST reply `"Error" "unknown"`, or `"Error" id "unknown"`
  when the command carries an id, and MUST NOT clear an in-flight command.
- A failure of a known command is `"Error" reason`, or `"Error" id reason`
  when the command carries an id. It is not an exception from an expression.
- Device payloads match `Bytes`. `Chunk` is `"Chunk" value` on a single
  stream and `"Chunk" id value` on a multiplexed stream. `End` is `"End"`
  or `"End" id`.
- Network and listen accepts run off the dispatcher. `run_until_idle` MUST
  NOT block on them and MUST NOT fire `send after`. The host delivers the
  reply when the operation finishes. A local file syscall MAY finish during
  the driver turn; the parent still observes the reply on a later turn.
- A checker replaces a driver by installing a script of reply batches.
  An empty batch leaves the command in flight. Snapshot MUST restore the
  remaining script and which commands are in flight. An operating-system
  handle MUST NOT resume across snapshot.
- `kith:dns` is not a module. It is the unknown-module example.
- Message names in `kith:net`, `kith:stdio`, `kith:fs`, and `kith:crypto`
  are strings, and so are the stream tags those modules share. `kith:http`
  and `kith:process` speak those words as registry symbols (`@Error`,
  `@Chunk`, `@Listen`, `@Env`). A string of the same spelling is an unknown
  command on those two modules and MUST reply `@Error`. Where a shared rule
  writes `"Error"`, those two modules use `@Error` with the same arguments.

## Net

`import "kith:net" Socket Listener;`

`Socket` is one TCP connection. The host resolves the host string. After
connect, both directions are the stream protocol.

| Down | Later up |
| --- | --- |
| `"Connect" host port` | `"Connected"` or `"Error" reason` |
| `"Pull"` | `"Chunk" bytes`, `"End"`, or `"Error" reason` |
| `"Push" bytes` | `"Pushed"` or `"Error" reason` |
| `"Close"` | `"Closed"` |

`Listener` is multiplexed by id. `"Listening" host port` reports the host
and bound port of that listen, including when the requested port is `0`.
A second `"Listen"` replies with that listen's pair.

| Down | Later up |
| --- | --- |
| `"Listen" host port` | `"Listening" host port` or `"Error" reason` |
| `"Pull" id` | `"Chunk" id bytes`, `"End" id`, or `"Error" id reason` |
| `"Push" id bytes` | `"Pushed" id` or `"Error" id reason` |
| `"Close" id` | `"Closed" id` |
| `"Stop"` | `"Stopped"` |
| | `"Connection" id` |

`bytes` matches `Bytes`. Connection ids are `c1`, `c2`, … in accept order.

## Http

`import "kith:http" Http Server;`

HTTP/1.1. The host parses the URL and frames the message. Bodies are the
stream protocol. The host follows no redirects. `https:` is TLS inside this
driver. The program does not import `kith:net` to use HTTP. Methods are
`GET` and `POST` only.

`Http` has one in-flight request. `@GET url` or `@GET url headers` replies
`@Status status headers` or `@Error reason`. `headers` is an object of
string to string, or null. `@Pull` then yields `@Chunk` of `Bytes` and
`@End`. `@POST url headers` replies `@Ready`. `@Push bytes` replies
`@Pushed`. `@BodyEnd` finishes the request body and replies `@Status`;
it is not `@Close`.

`Server` is multiplexed by id. `@Listen port` binds `127.0.0.1` and replies
`@Listening host port` with that host and the bound port. `@Listen host
port` binds that host and replies `@Listening host port` for that listen.
A second `@Listen` replies with that listen's host and port. An accepted
request is `@Request id method path headers`. `path` is the raw path and
query. A GET has no body: the first
`@Pull id` is `@End id`. `@Status id status headers` allows `@Push id
bytes`, `@Push id text`, and `@BodyEnd id`. `@Push id text` appends the
UTF-8 bytes of `text`. `@BodyEnd id` replies `@Responded id`. `@Stop`
replies `@Stopped`. Request ids are `r1`, `r2`, … in accept order.

## Http example

`examples/http.kith` imports `Server` from `kith:http` and `Process` and
`OptionalValue` from `kith:process`. It does not declare `component Http`.
On init it sends one `@Env` for `["HOST_NAME", "PORT"]`. The reply is
matched by `on up @Env { HOST_NAME: OptionalValue PORT: OptionalValue }`
with no name on the object. A null `HOST_NAME` is `127.0.0.1`. A null
`PORT` is `3000`. It MUST NOT `@Listen` until that reply is loaded, and
it MUST NOT exit when either name is null. The `@Listen` host and port
arguments are on their own lines. It MUST NOT store the host or port in
a field. `@Listening host port` `goto`s `Listening` before any
request is handled. Every `@Request` in that state is answered with
status 200, header `Content-Type` `text/plain`, `@Push` of the text
`ok\n`, and `@BodyEnd`.

## Hello world

`examples/hello-world.kith` is the Node.js synopsis server. It imports
`Server` from `kith:http` and `Process` and `OptionalValue` from
`kith:process`. On init it sends one `@Env` for `["HOST_NAME", "PORT"]`.
The reply is matched by
`on up @Env { HOST_NAME: OptionalValue PORT: OptionalValue }` with no
name on the object. A null `HOST_NAME` is `127.0.0.1`. A null `PORT` is
`3000`. It MUST NOT `@Listen` until that reply is loaded, and it MUST
NOT exit when either name is null. The `@Listen` host and port
arguments are on their own lines. It MUST NOT store the host or port in
a field. On `@Listening host port` it prints
`Server running at http://{host}:{port}/` from that message and `goto`s
`Listening`. Request handling is that state's `on up @Request`. Every request gets
`@Status` 200 with header `Content-Type` `text/plain`, `@Push` of the
text `Hello, World!\n`, and `@BodyEnd`.

## Stdio

`import "kith:stdio" Stdin Stdout Stderr;`

| Kind | Speaks |
| --- | --- |
| `Stdin` | `"Pull"` → `"Chunk" bytes`, `"End"`, or `"Error" reason` |
| `Stdout` | `"Push" bytes` → `"Pushed"`; `"Close"` → `"Closed"` |
| `Stderr` | the same writer protocol as `Stdout` |

`print` stays a diagnostic on process stderr as codec text. It is not
`Stderr`. Constructing `Stdout` takes over process stdout: pushed bytes are
the process stdout, and `kith run` MUST NOT also write the result record.
A run that ends without `"Close"` lets the host close the process stream
after the last `"Pushed"`.

## Fs

`import "kith:fs" File;`

One in-flight command. `"Open" path "write"` replaces the file. A missing
parent directory is `"Error" "missing parent"`. `"List"` is one directory
level, names only, sorted, as string chunks. `"Remove"` is one reply.

| Down | Later up |
| --- | --- |
| `"Open" path "read"` | `"Opened"`, then `"Pull"` → `"Chunk" bytes` or `"End"` |
| `"Open" path "write"` | `"Opened"`, then `"Push" bytes` → `"Pushed"`, `"Close"` → `"Closed"` |
| `"Remove" path` | `"Removed"` or `"Error" reason` |
| `"List" path` | `"Opened"`, then `"Pull"` → `"Chunk" name` or `"End"` |

Read chunks are at most 4096 octets.

## Encoding

`import "kith:encoding" utf8_encode utf8_decode base64_encode base64_decode hex_encode hex_decode;`

Pure. Invalid input MUST fail the expression. Byte arguments and results
match `Bytes`.

| Func | Returns |
| --- | --- |
| `(utf8_encode text)` | `Bytes` |
| `(utf8_decode bytes)` | text. Invalid UTF-8 MUST fail. |
| `(base64_encode bytes)` | standard alphabet, with padding |
| `(base64_decode text)` | `Bytes`. Bad alphabet or padding MUST fail. |
| `(hex_encode bytes)` | lowercase text |
| `(hex_decode text)` | `Bytes`. An odd length or a non-hex digit MUST fail. |

## Crypto

`import "kith:crypto" hmac_sha256 sha256_bytes Crypto;`

`(hmac_sha256 key data)` and `(sha256_bytes data)` are pure. `key` and
`data` match `Bytes`. The result matches `Bytes`. `sha256(text)` stays the
global for text.

`Crypto` `"Random" n` replies `"Bytes" bytes` of length `n`, or
`"Error" reason` when `n` is not a non-negative whole number. The draw uses
a snapshot stream that is not `(rand Lo Hi)`. Restore repeats the remaining
scripted bytes, or the next draw of that stream when no script is installed.

## Path

`import "kith:path" join dirname basename extname resolve;`

Pure. The separator is `/`. No function reads the cwd, the environment, or
the filesystem. `..` is not normalized.

| Func | Returns |
| --- | --- |
| `(join a b …)` | one path. An empty segment is skipped. An absolute segment restarts the result. |
| `(dirname path)` | the directory portion. `dirname("a")` is `"."`. `dirname("/")` is `"/"`. |
| `(basename path)` | the last segment. `basename("/")` is `""`. |
| `(extname path)` | the extension including the dot, or `""`. `extname(".gitignore")` is `""`. `extname("a.tar.gz")` is `".gz"`. |
| `(resolve base path)` | `path` when it is absolute, otherwise `path` joined onto `base`. |

## Process

`import "kith:process" Process OptionalValue RequiredValue;`

`OptionalValue` matches a string or null. `RequiredValue` matches a
string. Both are pattern rules. They do not read the process environment.

Argv and env are frozen at `World::start`. A later message sees the same
values. The checker replaces them. Argv is the program arguments after
`kith run`'s own flags and the program path. `@Exit code` ends the run
with that status after the turn and MUST NOT reply.

| Down | Later up |
| --- | --- |
| `@Argv` | `@Argv args` |
| `@Env names` | `@Env values` |
| `@Exit code` | no reply |

`names` is an array of one or more strings. `values` is one object. Each
requested name is a key, in request order, and a repeated name appears
once. A name in the frozen map is its string. A missing name is null. The
reply is that one object, not one message per name. An empty array, or a
call that is not one array of strings, replies `@Error "name"`. `@Env`
MUST NOT re-read the operating system.

## Why this design

- **The import is the driver.** A user-written `Http`, including the mock
  in `examples/health.kith`, stays a user component. The built-in is what
  a program gets when it imports `kith:http`, which is what
  `examples/http.kith` and `examples/hello-world.kith` do.
- **The stream is the body.** Net, HTTP, stdio, and files reuse `Pull`,
  `Chunk`, and `End` so a second byte type is not required.
- **The dispatcher does not block.** A connect or an accept finishes on a
  later turn. `run_until_idle` stays the non-blocking drain.
