# Buffer subset v1

`buffer` and `node:buffer` select the same module. Import `Buffer` explicitly;
there is no implicit global Buffer or npm package loader. API version 4,
JJS freeze version 25, and TPS profile `tps-default-v5` are required.

Supported:
- `Buffer.from(string[, encoding])`, `from(numericArray)`, and `from(Buffer)`.
  Copies input; numeric elements wrap modulo 256 after truncation; nonfinite numbers become zero.
- `alloc(size[, fill[, encoding]])`: always zero initialized; optional numeric,
  string, or Buffer fill repeats. Empty string fill leaves zeros.
- `concat(BufferArray[, totalLength])`: copies, truncates or zero pads.
- `isBuffer(value)`, `byteLength(string[, encoding])`, `byteLength(Buffer)`.
- Integer byte indexing and fixed `length`. Invalid canonical numeric indices
  read undefined; writes outside the range are ignored. Length assignment errors.
- `toString([encoding[, start[, end]]])`, with nonnegative integer offsets
  clamped to length; `toJSON()` produces `{type: "Buffer", data: [...]}`.

Encodings: UTF-8 (`utf8`, `utf-8`), Latin-1 (`latin1`, `binary`), `ascii`,
`hex`, and standard padded `base64`, case insensitive. UTF-8 decoding deliberately
replaces malformed sequences with U+FFFD, including a final incomplete scalar;
this is explicit decoding, never a transport conversion. ASCII decoding masks
bit 7. Latin-1/ASCII string encoding uses the low byte of each UTF-16 unit.
Hex requires complete valid pairs; base64 requires canonical standard encoding.
Forgiving Node hex/base64 forms, URL-safe base64, UTF-16 encodings, ArrayBuffer,
TypedArray overloads, slice/subarray, iterators, numeric-key enumeration,
negative offsets, and constructor calls are outside this subset and are not
advertised. Unsupported factory inputs/encodings fail explicitly.

Maximum buffer length: 16 MiB. Persistent bytes use a compact VM-owned Vec,
charged against the shared memory budget and serialized with the object.
Allocation and byte-processing paths consume fuel. Budget exhaustion is an
execution failure; invalid arguments produce TypeError or RangeError.
No unsafe/uninitialized allocator is exposed. Byte objects follow the VM's
existing module-object retention policy and count against its persistent budget.

`tests/bytes.js` is compared byte-for-byte with Node v24.13.0, interpreter,
and a probe with verified native completion. Tests also cover freeze/thaw,
new allocations after thaw, unsupported inputs, and split UTF-8 decoder state.
