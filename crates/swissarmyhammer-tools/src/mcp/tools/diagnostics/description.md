LSP diagnostics for your code, dispatched by `op`.

The `check` verb runs diagnostics over a scope and reports sharply — it always
includes the files you asked about, and of their one-hop dependents it folds in
only the ones that actually broke (never a project-wide dump):

- `check working` — diagnose files changed vs `HEAD` (the everyday op).
- `check file` — diagnose an explicit file path or glob.
- `check sha` — diagnose the files touched in/since a commit or range.

Each returns a `DiagnosticsReport { diagnostics, counts }`. Shared modifiers:
`severity` (minimum severity floor: `error`|`warning`|`info`|`hint`, default
`warning`), `settle_ms` (quiescence window), and `dependents` (fold in broken
dependents, default true).

The introspection ops read the LSP supervisor with no analysis:

- `list servers` — one status row per managed language server.
- `get server` — one server's status, by command name.
