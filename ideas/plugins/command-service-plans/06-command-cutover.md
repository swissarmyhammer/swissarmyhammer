# Plan 6 — Command Cut-over (terminal)

**Kanban project:** `command-cutover` · **Tier 3** · **Depends on:**
`builtin-commands` + `command-events` (and transitively everything below).

The big-bang milestone: move the frontend fully onto MCP and delete the old
YAML command architecture. The full-baseline e2e (all 62 commands through the
Command service) is the acceptance gate.

## Tasks

| Kanban id | Title | depends_on | Acceptance (one-liner) |
| --------- | ----- | ---------- | ---------------------- |
| `01KS36Y4NBDZMGH6QF963MD6FE` | Frontend: replace direct `invoke()` calls with `window` / `app` MCP dispatcher | window, app servers | Every non-transport `invoke()` migrated to MCP; window.rs + application.rs Tauri handlers deleted; `mcp_call`/`mcp_subscribe` kept; grep test enforces. |
| `01KS36Z0FQYYS7TZ005K5G5CDG` | Cut-over: delete `swissarmyhammer-commands` crate + YAML files + loader | all builtin-commands + command-events + backends + store + engine | Old crate + 12 YAMLs gone; no `use swissarmyhammer_commands::`; `cargo build/test --workspace` green; `full_baseline_e2e.rs` runs all 62 commands through the Command service. |

(The cut-over task's `depends_on` lists all 19 upstream tasks explicitly so the
board blocks it until the whole stack is done.)

## Key decisions baked in

- **Cut-over, not transitional**: the Command service, plugins, backends,
  events, and frontend land together; the YAML loader + `swissarmyhammer-commands`
  crate are deleted in the same change set.
- Pre-flight relocations (must happen before deletion): `UIState` →
  `ui-state` server (plan 3); `window_info` → `window` server if used.
- The `full_baseline_e2e.rs` test reads the catalog (`plugins.yaml`, plan 4)
  and asserts every one of the 62 commands registers with the right metadata and
  executes to the same effect as the YAML version — the cut-over contract.

## Cross-check

`kanban list tasks --filter '$command-cutover'` → expect exactly these 2 tasks.
