# Building JJS modules for Envhost and Microhost

This is a standalone handoff for an agent with no previous project context.
Its reference implementation is the session-backed Nodemailer module.
The code references and deployment details below were checked on 2026-09-17;
read the current files and dependency pins before making changes.

The task is to implement a specific requested module, connect it to the real
runtime, and prove that guest JavaScript can use it. This document does not
authorize unrelated changes or a production deployment.

## 1. Understand the environment

JJS is the JavaScript engine. TPS runs session environments using JJS.
Envhost owns the management API, session lifecycle, and trusted services.
Microhost is a deployed Envhost instance.

The application runs inside a session. It does not have the host's Node.js
runtime, npm installation, operating-system filesystem, or provider credentials.
A familiar import name does not imply full Node.js or npm-package compatibility.

For the modules described here, the JavaScript-facing implementation is a Rust
native module compiled into the runtime. Installing the upstream npm package is
not how this module becomes available.

| Repository | Working directory | Responsibility |
|---|---|---|
| JJS modules | /workspace/jjs-modules | Native compatibility adapters, shared contracts, module profile |
| JJS | /workspace/jjs | Engine and native-module API |
| TPS | /workspace/userlite-tps-2608 | Capability dispatch, guest execution, worker protocol |
| Envhost | /workspace/davidstarts-envhost | Trusted services, policy, persistence, management API, deployment |

Read applicable AGENTS.md files before editing each repository. Check Git status
and preserve existing changes and generated artifacts. In this checkout,
untracked target/ content is build output; do not stage it.

Use Vibrato runtime_shell_exec for project commands and Git authentication.
Prefer directory IDs from the current launch context; this session used 144 for
jjs-modules, 115 for JJS, 114 for TPS, and 145 for Envhost. Discover them again if
the environment differs. Do not assume another agent's local shell has runtime
credentials.

Start with these files, rather than reading entire repositories:

- [Existing module creation notes](README.md)
- [Error guidance](error-guidance.md)
- [Nodemailer adapter](../../modules/nodemailer/src/lib.rs)
- [Nodemailer crate manifest](../../modules/nodemailer/Cargo.toml)
- [Shared email contract](../../crates/jjs-email-contract/src/lib.rs)
- [Default module profile](../../crates/jjs-modules/src/lib.rs)
- TPS: crates/tps-runtime-jjs/src/email_tests.rs
- Envhost: crates/envhost-tps/tests/email.rs

The older creation notes include historical feature slices. Inspect current
module code and tests before treating those lists as today's compatibility
contract. Nodemailer currently has no separate CONTRACT.md; its adapter, shared
contract, and integration tests are the reference.

## 2. Decide whether the module needs a host service

A utility module implements guest-side behavior such as formatting or path
manipulation. It usually needs only module registration and guest tests.

A system module accesses a resource or performs an effect: email, networking,
storage, secrets, or another provider. Its adapter belongs in jjs-modules, but
the trusted resource and policy belong in the host.

Write down the requested surface before implementing:

- Exact import spellings and exported functions.
- Supported options, value types, return values, and Promise/callback behavior.
- Unsupported methods and fields, with useful explicit errors.
- Capability requirements, host policy, and session ownership.
- Input/output limits and the meaning of success.
- Resource lifetime and freeze/restore behavior.
- Idempotency and unknown-outcome behavior for external effects.

Do not promise complete compatibility with a large upstream package when only a
subset is implemented. Do not add masking fallbacks: invalid configuration,
missing permissions, unknown options, unavailable providers, and unsupported
features must fail explicitly.

## 3. Follow the Nodemailer path

A supported guest call looks like this:

```javascript
const mail = require('nodemailer').createTransport({ session: true });

const result = await mail.sendMail({
  idempotencyKey: 'order-123-confirmation',
  to: 'customer@example.com',
  subject: 'Order received',
  text: 'We received your order.'
});

// result.queueId identifies the queued message.
// A successful enqueue is not proof of delivery.
```

Reuse the idempotency key for a retry of that same operation; give a different
operation a different key. Do not run this example against a real recipient as
an incidental module test.

The actual boundary chain is:

```text
guest require('nodemailer') / sendMail()
  -> jjs-modules NodemailerModule
  -> jjs:email/enqueue host capability
  -> TPS runtime dispatcher
  -> TPS worker HostCall / HostCallCompletion
  -> Envhost parent validates the session and calls its registered handler
  -> Envhost email service owns persistence, policy, and provider access
  -> typed completion returns to the guest Promise
```

The relevant implementation points are:

| Boundary | File and search anchor |
|---|---|
| Adapter and continuation | jjs-modules/modules/nodemailer/src/lib.rs: NodemailerModule, request_host, resume |
| Shared message validation | jjs-modules/crates/jjs-email-contract/src/lib.rs: decode_message, EmailError |
| Public import registration | jjs-modules/crates/jjs-modules/src/lib.rs: tps_default_profile |
| Runtime registration/dispatch | TPS crates/tps-runtime-jjs/src/lib.rs: EMAIL_ENQUEUE, dispatch_email, set_email_transport |
| Cross-process contract | TPS crates/tps-protocol/src/lib.rs: HostCall::EmailEnqueue, HostCallCompletion::EmailEnqueue; src/email.rs |
| Worker bridge | TPS crates/tps-worker/src/lib.rs: set_email_transport, exchange_host_call |
| Parent binding and capability allowance | Envhost crates/envhost-tps/src/lib.rs: register_email_handler, enqueue_email, jjs:email/enqueue |
| Production service wiring | Envhost crates/envhost-network-harness/src/profile.rs: register_email_handler |
| Trusted email implementation | Envhost crates/envhost-embedded/src/email/ |

Use rg with these symbols; line numbers will change.

The guest submits message fields, not an environment ID or provider credentials.
The worker supplies its environment identity, and Envhost checks it against the
active environment before calling the handler. Apply that same boundary to a new
session-owned service; never trust a guest-supplied tenant/session identifier.

The adapter supports createTransport({ session: true }), not arbitrary SMTP
configuration. sendMail is Promise-based; callbacks and unsupported transport
methods return explicit errors. Attachments support captured content, including
Buffer input, rather than arbitrary file paths or URLs. Read the shared contract
for the exact current field set and validation.

## 4. Add the module in jjs-modules

For a module called example, the normal changes are:

1. Add modules/example/Cargo.toml and modules/example/src/lib.rs.
2. Add modules/example to the root Cargo.toml workspace members.
3. Add its path dependency to crates/jjs-modules/Cargo.toml.
4. Register its NativeModule implementation in tps_default_profile().
5. Update profile identity and catalog expectations deliberately.
6. Add a short module contract and focused tests.

A starting crate manifest is:

```toml
[package]
name = "jjs-module-example"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
jjs-module-api.workspace = true
```

Add other dependencies only when the implementation needs them. A shared typed
contract crate, like jjs-email-contract, is useful when both the adapter and
trusted host must validate the same payload. Do not duplicate divergent schemas.

Implement NativeModule against the API revision already pinned by the workspace.
Nodemailer demonstrates manifest, instantiate, call, resume, and event. Utility
modules can use a simpler existing module as their reference.

The manifest must declare:

- Stable module identity, semantic version, and implementation identity.
- MODULE_API_VERSION and the module's serialized state version.
- Every supported import spelling.
- All function keys, object-kind keys, dependencies, and resource declarations
  used by the implementation.
- Each host capability's ID, contract version, completion mode, and schema name.

Nodemailer currently uses org.jjs.nodemailer, implementation
jjs-module-nodemailer-v2, state_version 1, and the import nodemailer. Its capability
is jjs:email/enqueue, contract version 1, schema jjs.email.enqueue.v1, with yielding
completion. A new module needs its own identities and capability names.

Expose guest objects and functions through ModuleContext. Read guest values
through the API, validate them before effects, and charge fuel for bounded work.
Do not serialize arbitrary input with guest JSON.stringify and assume validation
is complete: it can discard unsupported values or invoke user code. Nodemailer
walks values, rejects functions/undefined/cycles, handles bytes explicitly, and
bounds traversal and message size.

Public imports must be present in both the provider and the selected catalog.
The current profile filters imports beginning with tps- out of its public
catalog. Adding an implementation alone does not guarantee application access.
Aliases such as example and node:example must be explicitly declared only if
supported; imports with different export objects may need separate identities.

At the checked revision, TPS_DEFAULT_PROFILE_ID is tps-default-v8 and profile
tests expect 15 catalog selections and 24 imports. These are baseline values,
not numbers to preserve after adding a public module. Update the profile ID and
expectations when changing the selected module set.

## 5. Add a host capability when needed

A native adapter must not directly acquire production credentials or bypass the
host's policy and resource ownership.

For a new system capability:

1. Define a bounded request and completion contract with stable error fields.
2. Declare the capability in the module manifest.
3. Register a matching descriptor in TPS and route it through its dispatcher.
4. Enforce the enabled-capability check before executing any effect.
5. If the resource belongs in the Envhost parent, add typed worker protocol
   variants and the matching worker/parent exchange.
6. Bind session identity from trusted runtime state and validate it in the parent.
7. Wire the service into the production Envhost profile, not just test fixtures.
8. Enable the capability only in profiles where it is intended to be available.

Follow the existing worker request/completion correlation checks. A request ID or
operation ID mismatch must fail explicitly. If the host might have accepted an
effect before the channel failed, report an unknown outcome and reconcile/retry
with the same idempotency key. Do not automatically create a new operation.

Nodemailer's request_host call uses ModuleContinuation(1) and requests a Promise.
resume converts successful host JSON into a guest value. Expected host failures
become ordinary guest Error values with code, field, and message, preserving the
original Error across Promise adoption. ContractViolation is for broken internal
protocol behavior, not routine user validation or a disabled service.

Test both await/try/catch and .then()/.catch(). An error is not correctly handled
if it becomes a successful result, loses its code, or escapes as an engine panic.

## 6. Respect persistence, settings, and lifecycle

Keep provider credentials and real resources host-owned. Use the existing
datastore adapters and migrations when durable state is required; do not assume
every adapter supports a feature. Validate the selected backend explicitly.

A module's advertised imports, identities, capability versions, continuation
state, and any host resource handles must remain meaningful after freeze/restore.
Do not retain process pointers, live sockets, or guest handles in arbitrary
global Rust state and expect checkpoints to preserve them. Rebind trusted host
transports through the established lifecycle.

A module change may make an old snapshot incompatible. Change identity/state/
contract versions as required, test the expected result, and provide an explicit
migration or rejection path. Do not suppress compatibility checks or claim an
old snapshot is compatible merely to get it running.

If the new module needs configurable limits, inspect Envhost's
docs/settings-system-plan.md and crates/envhost-embedded/src/settings/ first.
The generic settings system stores explicit configuration in MySQL/SQLite and
serves effective settings from memory; guest request handling must not query the
settings database or refresh on a cache miss.

Defaults, minimum/maximum bounds, override permission, and a session's override
are distinct. Changes are management-API/CLI operations; allowing an override
does not authorize the guest/model to change settings. Only introduce settings
required by the requested module. Do not move unrelated environment variables or
existing email settings as part of module work.

Keep separate limits separate. The email contract's current MAX_MESSAGE_BYTES
is 256 KiB; this is independent of the HTTP response buffer setting. Microhost's
HTTP default was explicitly set to 256 KiB with a 512 KiB policy maximum and
session overrides disabled at this handoff. Those deployment values are mutable;
they are not new module defaults or hardcoded constants to copy.

## 7. Prove the complete path with focused tests

Choose tests for the changed boundaries. A unit test that checks a manifest does
not prove that production guest code can use the module.

| Test boundary | Required evidence when applicable |
|---|---|
| Contract/adapter | Supported values, wrong types, unknown fields, exact size boundaries, useful errors |
| Module profile | Import resolution, aliases, catalog and identity expectations |
| TPS guest runtime | Real JavaScript invokes the production dispatcher; await and catch preserve results/errors |
| Capability/session isolation | Disabled capability has no effect; forged session fields fail; no cross-session access |
| Worker protocol | Real child worker reaches the Envhost handler and returns typed success/failure |
| Lifecycle | A module object created before freeze still behaves correctly after restore |
| Durable effects | Duplicate request deduplicates, conflicting reuse fails, unknown outcome is explicit |
| Execution mode | Interpreter/JIT coverage where the module touches execution behavior and those modes are supported |

Nodemailer reference tests:

- TPS crates/tps-runtime-jjs/src/email_tests.rs covers disabled capability,
  forged environment fields, Express handlers, freeze/restore, attachments, and
  structured errors.
- Envhost crates/envhost-tps/tests/email.rs exercises the actual child worker
  bridge with a fake trusted handler.
- Envhost crates/envhost-embedded/src/email/tests.rs covers the trusted service.
- jjs-modules/crates/jjs-email-contract/src/lib.rs contains contract tests.

Use fake provider handlers for ordinary tests; do not send real emails or make
billable external calls unless that verification is explicitly authorized.

Examples of focused commands for the existing reference implementation:

```sh
# /workspace/jjs-modules
cargo +1.94.1 test --locked -p jjs-module-nodemailer -p jjs-email-contract
cargo +1.94.1 test --locked -p jjs-modules default_profile_has_stable_identity_and_complete_catalog

# /workspace/userlite-tps-2608
cargo +1.94.1 test --locked -p tps-runtime-jjs email_tests

# /workspace/davidstarts-envhost
cargo +1.94.1 test --locked -p envhost-tps --test email
```

These are reference tests, not a requirement to rerun email tests for every
unrelated utility module. Add/use the equivalent focused tests for the new
module. Rust 1.94.1 built the current release; the older default 1.91 toolchain was
insufficient for its Cranelift dependency.

## 8. Carry the change through Git pins

Editing a sibling checkout does not change what Cargo builds in a consumer.
These repositories use pinned Git dependencies.

At the checked baseline:

- jjs-modules pins JJS and jjs-module-api to the same JJS revision.
- TPS pins jjs-modules in its root Cargo.toml.
- TPS also pins jjs-email-contract in crates/tps-protocol/Cargo.toml.
- Envhost pins tps-core, tps-protocol, and tps-worker in its root Cargo.toml.

Read current manifests and Cargo.lock; do not use an old hash from a conversation.

The normal release order is:

1. If engine/API changes were necessary, test, commit, and push JJS first.
2. Keep JJS engine/API pins compatible in modules and TPS; test, commit, and push
   jjs-modules.
3. Update TPS's affected jjs-modules/shared-contract pins to the pushed revision;
   regenerate its lockfile, run focused integration tests, commit, and push.
4. Update Envhost's TPS pins together to the pushed TPS revision; regenerate its
   lockfile, test the affected integration, commit, and push.
5. Build deployment artifacts with --locked from those committed revisions.

Temporary local Cargo patches can help development, but the final consumer test
must use the committed Git pins. Do not leave accidental local paths/patches or
mixed revisions of a shared contract in the release. Changes to dependencies
require an intentional lockfile update before running --locked checks.

## 9. Deploy only when requested

Microhost is a remote systemd deployment. At this handoff it uses
microhost.service, host/worker binaries under /opt/microhost/bin, and retained
releases under /opt/microhost/releases. Its public URL is
https://microhost.userlite.io. Rediscover the saved SSH connection and current
service configuration; the local Envhost .env in this workspace targets Kivo,
not Microhost.

For an authorized deployment:

- Build a compatible host/worker pair for the target architecture; deploy the CLI
  too if it changed.
- Apply required schema migrations with the proper migration credentials.
- Preserve the instance's explicit configuration and record release revisions.
- Verify artifact checksums and stage the complete release before activation.
- Restart through Vibrato processes_list/processes_manage using its exact process
  ID. If the remote service is not managed there, obtain explicit permission for
  the alternate restart method; do not substitute an ad hoc process manager.
- Verify readiness and one focused real guest call on a dedicated test session.
- Test both success and a relevant policy/validation failure without touching
  existing user applications.
- Restore temporary test configuration and stop the test session.

A representative build for this deployment is:

```sh
# /workspace/davidstarts-envhost
cargo +1.94.1 build --release --locked   -p envhost-network-harness -p envhost-tps   --bin envhost-network-harness --bin envhost-tps-worker
```

A successful build or require() call alone does not prove deployed integration.
The new module must be present in the deployed worker and connected to any
required parent service. Respect snapshot compatibility for existing sessions.

Never print secrets, include them in test evidence, commit credentials, or expose
them to guest code. Deployment access does not authorize unrelated service
changes.

## 10. Completion checklist and next-agent prompt

A completed module handoff names:

- The exact supported API and explicit unsupported behavior.
- Changed repositories and pushed commits.
- Where the module is registered and its host capability is enforced.
- Focused tests run and any unverified boundary.
- Snapshot compatibility and migration requirements.
- Whether it was deployed, where, and what live test actually passed.

Do not report provider delivery when only enqueue was tested, or claim deployment
when only source pins were updated.

Copy this prompt when assigning the next module:

> Implement [module name] for Envhost/Microhost using
> docs/module-creation-system/envhost-module-handoff.md in jjs-modules.
> The required JavaScript API is [imports, methods, examples].
> The supported options and behavior are [scope].
> Host resources/policy requirements are [requirements].
> Explicit exclusions are [out of scope].
> Deployment is [requested / not requested].
> Read the current code and applicable AGENTS.md files, preserve unrelated
> changes, implement the complete required path, run focused tests, and commit
> and push each changed repository. Report the exact compatibility surface and
> any boundary that remains unverified. Do not create masking fallbacks.
