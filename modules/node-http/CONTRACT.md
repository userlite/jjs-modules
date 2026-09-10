# HTTP input, output, bytes and headers contract v4

Implementation `jjs-module-node-http-v4`, state version 6, module API 4;
requires the selected `org.jjs.node-buffer` v1 implementation. Shipping TPS
profile: `tps-default-v6`. Old profiles/snapshots are rejected.

Request bodies remain bytes through TPS. The module event payload carries
`bodyBase64` and `headersJson` (an ordered array of string pairs); these replace
the old text body/header object payload. No automatic UTF-8 decoding occurs.
Default `data` chunks are actual Buffers. The legacy buffered dispatch used by
MCP retains `request.body`, slices it into 16 KiB chunks, and emits end after
synchronous delivery. Streamed requests expose `streamedInput: true` and leave
`request.body` undefined; callers must consume data/end or readable/read.
`setEncoding('utf8'|'utf-8'|'latin1'|'binary'|'ascii')` is allowed before data
starts. UTF-8 retains incomplete scalars between real chunks and deliberately
emits U+FFFD for malformed sequences/final incomplete input. Decoder residual
bytes are VM private state, not a host-side text replacement.

`rawHeaders` retains the supplied name case, order, and duplicates.
`headersDistinct` groups arrays by lowercase name. `headers` uses Node's
normalized rules: Set-Cookie arrays, Cookie joined with `; `, singleton fields
keep the first value, other duplicates join with `, `. Public HTTP ingress
uses its HTTP library's normalized field-name case; it preserves repeated
values. MCP supports both header objects and ordered string-pair arrays.

Response `setHeader` and `writeHead` accept string or nonempty string-array
values. Replacing a name removes its old values case-insensitively. Wire headers
remain ordered pairs, including separate Set-Cookie values. Names require ASCII
HTTP token characters; values support visible ASCII and tab, with CR/LF/control
characters rejected. Bounds: 256 entries, 256 bytes/name, 16 KiB/value,
64 KiB combined name/value bytes. Non-ASCII header values are unsupported.
Invalid headers fail deterministically and do not update the stored headers.

Headers and status commit on writeHead, first write, flushHeaders, or end.
headersSent becomes true; late setHeader/writeHead throw. Later statusCode
assignments cannot alter the committed wire status. writeHead's optional status
message is accepted as in the prior subset; the host serializes its standard
status reason. Final statuses 200–599 are supported; informational responses
are unsupported. HEAD, 204, and 304 suppress all body bytes. This subset removes
Content-Length and Transfer-Encoding for 204/304; HEAD retains explicit headers.
`end` and `write` accept strings or Buffers (numeric arrays are no longer a
private byte substitute). Buffered end copies the bytes before returning.

Streaming capability `jjs:http/stream` is version 2 / `jjs.http.stream.v2`:
start carries status and JSON ordered header pairs; write/end carry strict
standard base64 bytes, or undefined for no final chunk. Existing connection,
request, sequence, drain, close, and queue-limit checks still apply. TPS restores
connection snapshots inside the service's intern-table scope. Input streaming uses the following additional protocol.


## Credited input protocol v1

Start (event 5) carries empty body bytes plus the existing metadata and identities.
Data (6), end (7), abort (8), and error (9) carry version 1, connectionId,
requestId, and a strictly increasing sequence starting at 1. Data is strict
base64, nonempty, at most 16 KiB. State (10) reports next sequence, input credit,
and inputEnded, or raises a captured async rejection. Wrong identity, version,
sequence, chunk size, or credit is a boundary error.

There is one queued Buffer per request. A pause or occupied queue grants zero
credit. Adding a data listener selects flowing mode; resume clears explicit
pause and drains the queued chunk. Without a data listener, readable fires and
read() removes the whole queued chunk, or returns null. Sized reads, piping,
unshift, byte-mode changes after data, and full Node Readable emulation are not
in this subset. Legacy buffered requests reject pause/resume/read explicitly.
End requires the queued input to have been consumed and flushes decoder state
before emitting end once. No data/end/abort/error is accepted after end, abort,
or error. Close is allowed after any of these transitions, exactly once.
Closing incomplete input emits aborted once unless abort/error already ran;
then request and response close fire. Late input and drain are rejected.
`req.connection.destroy()` and `req.socket.destroy()` throw the explicit
`node_http_socket_destroy_unsupported` error.

## Completion and host flow control

The worker exposes OpenHttpExchange, PollHttpExchange and CloseHttpExchange.
Poll may deliver one input event and acknowledge one previously delivered output
batch. Listener return does not complete the request: response end and output
acknowledgment do. Returned listener promises attach rejection hooks; jobs and
timers resume through TPS. Rejection, input error, timeout and disconnect close
and retire the exchange. Unreturned promises are outside this capture contract.
Close cleanup cancels remaining timers and subscriptions; cleanup is not an
unbounded asynchronous extension of the request lifetime.

Worker bounds: 32 exchanges, 16 MiB input per exchange, 16 KiB input chunks,
16 KiB output high-water mark, 64 KiB hard output limit, 30 seconds elapsed
lifetime, plus the service execution budget. A false write keeps output charged
until the client acknowledges that exact batch. Later queued writes stay charged;
drain fires only with available capacity and an open response. Empty writes do
not create acknowledgment batches. Exceeding limits is an explicit failure.

Envhost streams the incoming HTTP body instead of collecting it first. It pulls
input only with credit, forwards output through a one-slot channel, and waits
for that channel to be consumed before acknowledging output. It retains at most
one incoming transport frame plus the credited chunk; its total input limit is
8 MiB. A slow reader/writer or disconnected body cannot produce a successful
truncated response: body errors are delivered after queued chunks drain.

Active exchanges use TPS's existing isolated connection snapshots: request-local
VM changes are not merged into the service VM. Shared host storage is the way to
persist changes across these connections. MCP buffered dispatch keeps its existing
service-state behavior. Express JSON accepts both streamed and buffered input;
see `../express/CONTRACT.md` for byte limits, UTF-8, errors, and collection semantics.
These restrictions must be included when certifying application/module combinations.
The internal read-only `_bodyInputState` accessor reports available, consumed,
decoded, ended, or terminal input so middleware cannot silently parse a tail.

Active exchange freeze/checkpoint returns an explicit boundary error. Envhost
keeps durable journal intent open and defers recovery checkpoints until the final
exchange closes. A crash with such intent is an interrupted operation, not a
silently replayed HTTP request. Freeze succeeds after cleanup. Service stop and
replacement close their active exchanges. Old profile/state snapshots fail the
existing compatibility checks. No managed deployment is part of this iteration.
