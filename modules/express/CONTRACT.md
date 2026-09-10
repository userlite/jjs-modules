# Express JSON subset

Express state version 9 requires fresh compatible snapshots; HTTP state version 6
adds the internal raw-input eligibility accessor. Existing implementation identities
remain unchanged; state versions participate in the module-set fingerprint.

`express.json({limit, strict})` supports buffered bodies and credited streamed input.
The existing default is 100 KiB, strict is true, and other options are rejected.
Only `application/json` is matched, case-insensitively, ignoring parameters for
media-type matching. Other media types pass through without collection. Content
encoding must be absent or identity; charset must be absent or UTF-8. Empty input
produces `{}`; BOM-prefixed input remains malformed. Strict mode accepts objects
and arrays; `strict:false` also accepts JSON primitives.

The collector retains VM-owned byte chunks, whose storage and array allocations
are charged by the runtime, up to the configured raw-byte limit. It assembles the
complete document once at end, then validates UTF-8 and parses JSON. Split UTF-8
sequences work; invalid bytes are never replaced. Temporary assembly is bounded
by the received byte count; decoding and parser/object allocation add overhead.
This is complete-document collection, not an incremental JSON parser. Transport
body, frame, deadline and VM resource limits still apply.

Named parser errors reach `next(error)` with `status` and `statusCode`:

| Condition | Name | HTTP status |
| --- | --- | --- |
| Malformed JSON or invalid UTF-8 | ExpressJsonSyntaxError | 400 |
| Strict-mode primitive | ExpressJsonStrictError | 400 |
| Byte limit exceeded | ExpressJsonLimitError | 413 |
| Unsupported encoding or charset | ExpressJsonEncodingError | 415 |
| Consumed, ended, decoded or terminal stream | ExpressJsonRequestError | 500 |

The default Express error handler honors valid 400–599 error statuses. Resource
failures propagate as runtime failures, rather than becoming syntax errors.
Exact-limit input succeeds; overflow fails before retaining excess bytes and
pauses input while the existing response lifecycle delivers the error response.
A validated numeric Content-Length above the limit is rejected before collection.

Parser completion uses an Express-owned private marker, separate from `req.body`.
Repeated middleware skips successful parsing; application assignment to `req.body`
does not skip streamed collection. Saved continuations resume the remaining
handlers and layers once and release their references on continuation or close.
The collector removes only its own listeners and clears all saved byte/handle
references before success, failure or cancellation. Abort and transport input
failure cancel through the existing terminal exchange lifecycle; no partial JSON
is parsed or fresh response attempted on a terminated input connection.

Streamed requests continue to use isolated VMs. Active requests cannot freeze.
The focused TPS tests verify collector Buffer reclamation and bounded per-request
allocation cost after GC. Aggregate shared service accounting still grows across
retired isolated VMs; this existing runtime retirement/accounting boundary is not
fixed here, and the plan's total retained-memory plateau criterion remains open.
No managed deployment is included.


## URL-encoded forms (Iteration 6)

`express.urlencoded()` and `{extended:false, limit}` implement flat Express 5.1.0
forms. The default limit is 100 KiB, with at most 1000 ampersand-separated
parameters (overflow is 413). Unknown options and `extended:true` fail at setup.
Bracket names remain literal; duplicate keys form arrays; empty fields are skipped;
empty values remain strings. Plus means space. Valid percent escapes decode UTF-8;
malformed percent/UTF-8 components remain encoded, matching Express/qs. Raw invalid
UTF-8 uses replacement characters. `__proto__` is ignored as in Express/qs.
Only the URL-encoded media type selects this parser; UTF-8 and identity encoding
are supported. Other charsets/compression reach the explicit 415 error path.
The shared JSON collector supplies limit/encoding/input errors and cleanup;
parameter overflow is `ExpressFormParameterLimitError`. Repeated successful
parsers share a completion marker and never consume the same body twice.
State version 9 adds the form collector discriminator and native functions;
module-set fingerprints reject earlier snapshots rather than migrating them.

The historical isolated-VM accounting note above predates the shared-state runtime;
current composition acceptance uses the shared service VM (see the Iteration 6 ledger).
