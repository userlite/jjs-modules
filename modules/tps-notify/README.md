# `tps-notify`

`tps-notify` is the least-authority JJS surface for durable owner notification.
It never delivers a message inside the guest VM. `send(request)` yields one
versioned `jjs:notification/send` host request and resumes with the host's
durable, redacted receipt.

```js
const notify = require("tps-notify");

const accepted = await notify.send({
  operationId: "guardian-42-alert-17",
  destination: { id: "owner-primary", generation: 1 },
  subject: "Watchtower condition detected",
  body: "The monitored filing status changed.",
  contentType: "text/plain; charset=utf-8",
  evidenceReceiptSha256: "a".repeat(64)
});
```

The destination is an opaque handle provisioned by the host. Addresses,
provider credentials, environment identity, delivery time, retry state,
attempt identity, and terminal receipt fields are not accepted guest inputs.
A successful call means the request and accepted receipt are durable; it does
not claim that an external provider has already delivered the message.
