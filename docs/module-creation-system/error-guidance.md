# Error guidance

Runtime failures are opportunities to move an agent directly toward supported code. Errors must remain honest failures: guidance explains the valid next action, but never retries, substitutes behavior, or hides a configuration or programming mistake.

## Response shape

When the runtime knows the failed boundary, return four small pieces:

1. A stable error code when the boundary supplies one.
2. The exact condition that failed.
3. One short, verified correction.
4. An example only when it removes meaningful ambiguity.

Public error codes describe the failed capability or domain. Do not expose internal runtime or implementation prefixes to agents.

Prefer:

```text
SQLITE_CANTOPEN: absolute database paths are not allowed; use a relative session path such as 'app.db'
```

Avoid:

```text
Database open failed
```

## Exact guidance before fuzzy guidance

Use deterministic mappings whenever the runtime knows the condition:

- A rejected absolute database path should recommend a relative session path.
- An unavailable `sqlite` or `node:sqlite` import should recommend the installed `better-sqlite3` module.
- An unsupported API should name the API and state whether retrying can succeed.

Fuzzy suggestions are a later layer for genuinely ambiguous failures. They must be based on a small, reviewed catalog of observed failures and verified corrections. They must never guess when the runtime can identify the exact condition.

## Fix compatibility defects instead of documenting around them

If ordinary JavaScript or a supported module API is missing, implement it. For example, a normal `Date` method such as `toISOString()` belongs in JJS; agents should not receive advice to avoid standard JavaScript.

## Ownership

- JJS owns JavaScript-language and standard-built-in compatibility.
- Module adapters own API validation and unsupported-feature errors.
- TPS owns host policy errors such as session paths and capability restrictions.
- Module resolution owns unavailable-package guidance based on the actual installed catalog.
- The session transcript must preserve the complete structured error so later agents and developers can see the same actionable information.

## Initial catalog

| Observed condition | Correction |
| --- | --- |
| Absolute SQLite database path | Use a relative session path such as `app.db`. |
| `require('sqlite')` | Use `require('better-sqlite3')`. |
| `require('node:sqlite')` | Use `require('better-sqlite3')`. |
| Missing standard `Date` instance method | Implement the method in JJS. |

Add guidance only after the correction is verified against the runtime. Keep messages short enough to be useful in the agent's immediate context.
