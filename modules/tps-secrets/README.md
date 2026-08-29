# `tps-secrets`

`tps-secrets` is the JJS guest API for environment-owned secret material. Secret
bytes cross the host boundary only during `importSecret`; every later operation
uses an opaque, generation-bound handle.

Every method accepts one exact options object, yields a typed host request, and
returns the host's version-1 JSON completion:

```js
const secrets = require("tps-secrets");

const handle = await secrets.generate({
  operationId: "create-signing-key",
  label: "watchtower-signing-key",
  length: 32,
  policy: {
    allowedUses: ["verify", "hmac_sha256"],
    maxInputBytes: 65536
  }
});

await secrets.verify({
  operationId: "verify-webhook",
  handle,
  candidate: [1, 2, 3]
});
```

Exports: `generate`, `importSecret`, `rotate`, `deleteSecret`, and `verify`.
The host injects the contract version and environment identity. Applications
cannot select another environment or read stored secret bytes.
