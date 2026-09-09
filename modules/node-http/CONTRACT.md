# HTTP bytes and headers contract v3

Implementation `jjs-module-node-http-v3`, state version 4, module API 4;
requires the selected `org.jjs.node-buffer` v1 implementation. Shipping TPS
profile: `tps-default-v5`. Old profiles/snapshots are rejected.

Request bodies remain bytes through TPS. The module event payload carries
`bodyBase64` and `headersJson` (an ordered array of string pairs); these replace
the old text body/header object payload. No automatic UTF-8 decoding occurs.
`request.body` and default `data` chunks are actual Buffers. Buffered input is
split into 16 KiB chunks; empty input emits no data and then end. This is not
incremental network input (iteration 5). All listeners remain synchronous.
`setEncoding('utf8'|'utf-8'|'latin1'|'binary'|'ascii')` is allowed before data
starts. UTF-8 retains incomplete scalars between chunks and deliberately emits
U+FFFD for malformed sequences/final incomplete input. Decoder state lives for
the synchronous delivery; yielding from a listener remains an explicit error.

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
connection snapshots inside the service's intern-table scope. Response streaming
is already supported; true request transport streaming remains iteration 5.
