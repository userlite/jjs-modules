# Module creation system notes

These are working notes, not a finished specification. Update them after building modules so the rules come from real integration work.

- [Error guidance](error-guidance.md) explains how failures give agents short, verified corrections without masking the error.

## Two module shapes

### Utility module

- Implements deterministic guest-side behavior.
- Does not request host capabilities.
- Examples: path manipulation, formatting, validation.

### System module

- Presents a familiar JavaScript API over host-owned state or effects.
- Declares a versioned host capability contract.
- Requires host policy, resource limits, typed errors, checkpoint behavior, and end-to-end guest tests.
- Examples: filesystem, databases, networking, secrets.

## System-module boundaries learned so far

1. Put the JavaScript compatibility adapter in `jjs-modules`.
2. Keep the real resource in the host; `fs` can only reach the session VFS, never the machine filesystem.
3. Register every import spelling explicitly (`fs`, `node:fs`, `fs/promises`, `node:fs/promises`).
4. Use separate module identities when two imports need different export objects.
5. Give unsupported features a structured `JJS_UNSUPPORTED_FEATURE` error with `retryable: false`; never substitute different behavior.
6. Test through real guest JavaScript and the production host dispatcher, not only adapter unit tests.
7. Persist module identities and capability contracts so freeze/wake cannot silently change the available API.
8. Keep host resources service-scoped when guest objects are reused by request handlers; checkpoint both the guest handle and the host resource identity together.
9. Return expected resource failures through the capability contract, then recreate them as normal JavaScript errors with stable `name`, `code`, and `message` fields. Do not let database errors escape as native-module contract violations.
10. Framework adapters must pass the original JavaScript error to registered error middleware. If no middleware handles it, development responses must be structured and actionable instead of replacing it with a generic message.

## Node filesystem first surface

The first `fs` slice wraps existing session-VFS operations:

- UTF-8 `readFile` and `readFileSync`
- String `writeFile` and `writeFileSync`
- `readdir`, `stat`, `mkdir`, `exists`, and `access`
- Callback, synchronous, and promise shapes

Binary results require a real `Buffer` module. File descriptors, streams, watchers, recursive directory creation, append, copy, rename, and deletion require additional honest host contracts and remain explicit unsupported features until built.
# Module creation system

- [Error guidance](error-guidance.md): how runtime failures should give agents short, verified corrections without masking errors.
