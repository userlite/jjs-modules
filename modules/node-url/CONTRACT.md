# URL contract v1

`url` and `node:url` export the same object with `URL`, `URLSearchParams`, and `parse`. This is a bounded subset, not the complete Node URL module. No host capabilities, sockets, DNS, or clocks are used. Differential fixtures use Node v24.13.0; WHATWG parsing uses Rust `url` 2.5.8 and form encoding uses `form_urlencoded` 1.2.2.

`new URL(input[, base])` accepts string input and an optional string base. Invalid URLs or bases throw. Getters: `href`, `origin`, `protocol`, `username`, `password`, `host`, `hostname`, `port`, `pathname`, `search`, `hash`, `searchParams`. Setters: `href`, `pathname`, `search`, `hash`; other setters explicitly fail. `toString()` and `toJSON()` serialize the URL. Search parameters retain their identity and stay linked through URL changes, parameter changes, and freeze/thaw.

`new URLSearchParams([query])` accepts a string, optionally beginning with `?`, or no argument. Supported: `get(name)`, `getAll(name)`, `has(name)`, `append(name, value)`, `set(name, value)`, `delete(name)`, `sort()`, `toString()`, and read-only `size`. Names and values must be strings. Duplicate ordering and stable UTF-16 sorting match Node. Form decoding maps plus to space, preserves malformed percent escapes as text, and replaces invalid UTF-8 with U+FFFD; this is the standard decoding rule. Serialization uses plus for spaces and percent-encodes literal plus signs.

Record/iterable constructors, iterator APIs, optional value filters for `has`/`delete`, static URL helpers, file URL utilities, and prototype/subclass compatibility are unsupported. Unsupported methods/options fail explicitly. Accessors are instance properties; prototype/descriptor layout is not claimed.

`parse(input[, parseQueryString[, false]])` is a **separate legacy parser**, preserving dot segments, numeric ports, query text, and the legacy nullable property shape. It accepts relative request targets and absolute HTTP(S) URLs with ASCII letter/digit/dot/hyphen hostnames and optional numeric ports. Query parsing uses the querystring contract with the default 1000-field limit and a null-prototype result. Without query parsing, `query` is raw text or null. Non-HTTP(S) schemes, userinfo, IPv6/non-ASCII authorities, embedded control characters, and `slashesDenoteHost: true` fail explicitly. Legacy `format` and `resolve` are not exported. Authority-free `//path` stays a path under the supported false flag.

Inputs and serialized outputs are limited to 65,536 UTF-8 bytes; search parameters are limited to 4,096 pairs. Violations fail explicitly, and mutations check limits before changing saved state. Parser work charges fuel; all persistent state belongs to the VM and survives compatible snapshots. Versioned module manifests and `tps-default-v4` reject incompatible compositions.

Sources: [Node URL API](https://nodejs.org/docs/latest-v24.x/api/url.html), [WHATWG URL](https://url.spec.whatwg.org/).
