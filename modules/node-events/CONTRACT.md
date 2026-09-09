# Events contract v1

`events` and `node:events` export the same constructor, also available as `.EventEmitter`.
Supported instance methods: `on`/`addListener`, `once`, `removeListener`/`off`, `removeAllListeners`, `listeners`, `listenerCount`, `eventNames`, and synchronous `emit`.

Events are strings up to 1024 UTF-8 bytes. Each emitter permits at most 1024 registrations; exceeding the limit throws before mutation. Constructor options, symbol events, promise helpers, prepend methods, raw wrappers, and subclass/prototype compatibility are not part of this contract. Unsupported calls fail; there is no automatic retry or ignored listener.

Listeners run in registration order on a snapshot. Additions affect the next emission; removals do not remove callbacks from an active snapshot. Duplicate listeners are retained, and removal removes the most recent matching registration. Once records are removed and marked fired before invocation, including recursive emissions. Inspection returns copies with original callback identities; event names retain JavaScript property ordering.

Ordinary functions receive the emitter as `this`; arrow functions retain their lexical receiver and bound callbacks retain their bound receiver. Guest throws propagate unchanged and stop dispatch. An unhandled `error` event throws its object argument (or an Error if there is no object). Errors from execution budgets remain runtime errors. A synchronous listener cannot suspend on a host operation; this is an explicit boundary error. Promise rejection capture is unsupported.

All tables, records, callbacks, and once state live in the VM and survive compatible freeze/thaw. Native loops charge fuel. Invalid saved listener state is an error. Module API v2 supplies callback identity and HTTP state v3 / implementation v2 rejects old fingerprints.

HTTP uses this same implementation: server events `request`, `listening`, `error`, `close`; request events `data`, `end`, `close`, `error`; response events `drain`, `close`, `error`. Only existing host lifecycle events are delivered automatically; registering `error` does not swallow a thrown listener exception. Input remains the existing single UTF-8 chunk contract (including empty text), followed by one end emission. Network input streaming is a later iteration.

The deterministic fixture in `tests/fixtures/listeners.js` is compared with Node v24.13.0; runtime tests also cover bounds and freeze/thaw. The neighboring HTTP tests cover errors, next-request recovery, cold/warm callbacks, synchronous yield rejection, and close/drain after freeze.
