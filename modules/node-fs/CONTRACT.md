# Filesystem byte contract (Iteration 6)

`readFileSync`, callback `readFile`, and promise `readFile` return the selected
Buffer type when encoding is omitted/null; explicit UTF-8 returns a string.
Their write equivalents accept strings or Buffers. Other encodings/options fail
explicitly. VFS paths and read/write capabilities remain authoritative.

Read/write host descriptors now use contract version 2 and request schema v2.
A binary read adds a `base64` wire-encoding argument after the path; a binary write
adds it after encoded data. This is an explicit internal transport representation:
the VFS stores original bytes and guest reads return Buffers. Invalid wire data
fails; it is never interpreted as text. String calls retain their existing shape.
Other filesystem capabilities retain version 1. FS and fs/promises state version
3 and the Buffer module dependency participate in snapshot fingerprints.

TPS Iteration 6 tests certify exact non-UTF-8 bytes through sync/callback/promise
calls and combined upload/download, including compatible freeze/wake. Host limits
charge encoded transport bytes conservatively; VFS errors propagate to the normal
sync, callback, or promise error path before application state is published.
