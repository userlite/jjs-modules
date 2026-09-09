# Querystring contract v1

`querystring` and `node:querystring` export one object. `parse`/`decode` are aliases; `stringify`/`encode` are aliases. No host capabilities are required. Fixtures use Node v24.13.0.

`parse(input[, sep[, eq[, {maxKeys}]]])` returns a null-prototype object. The first value is a string; repeated keys become arrays in encounter order, including `__proto__`, `constructor`, and `toString`. Empty values are retained; empty fields are skipped. Plus decodes to space. Percent decoding deliberately follows Node's malformed-input replacement behavior: invalid escapes remain text and invalid UTF-8 becomes U+FFFD. As in Node, non-string input returns an empty result.

`stringify(object[, sep[, eq[, {}]]])` visits enumerable own string keys in JavaScript order. Array values produce repeated fields. Strings, finite numbers, and booleans serialize; other values and non-finite numbers serialize as empty text, as specified by Node. Empty arrays produce no fields. Spaces encode as `%20`; literal plus encodes as `%2B`. Non-object input produces an empty string.

Separators default to `&` and `=` and may be 1–16 UTF-8 bytes. Empty separators and non-string separators are explicitly unsupported. `maxKeys` defaults to 1000 and accepts integers 0–4096; zero removes Node's configurable cutoff but retains the hard resource limit. Positive limits deliberately stop after that many fields, matching Node. Custom encoding/decoding callbacks, unknown options, negative/non-integer limits, and extra arguments fail explicitly. Standalone `escape`/`unescape` helpers are not exported.

Input and serialized output limits are 65,536 UTF-8 bytes. The hard limit is 4,096 fields (or enumerable stringify keys); exceeding a hard limit fails. Work charges fuel. Pure native parsing has no suspension or host event contract.

Source: [Node querystring API](https://nodejs.org/docs/latest-v24.x/api/querystring.html).
