# `jjs-crypto`

`jjs-crypto` provides host-recorded cryptographic operations for deterministic,
freezable JJS programs. The host owns entropy and secret material; the guest
receives only replayable results.

```js
const crypto = require("jjs-crypto");

const nonce = await crypto.randomBytes({
  operationId: "delivery-nonce",
  length: 32
});

const digest = await crypto.sha256({
  operationId: "snapshot-digest",
  message: [1, 2, 3]
});

const signature = await crypto.hmacSha256({
  operationId: "sign-notification",
  handle,
  message: [1, 2, 3]
});
```

Exports: `randomBytes`, `sha256`, and `hmacSha256`. Every method accepts one
exact options object and yields a version-1 host request. Reusing an operation
ID with identical input replays its recorded result; conflicting reuse fails.
