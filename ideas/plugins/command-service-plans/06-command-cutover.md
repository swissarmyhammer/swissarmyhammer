# Plan 6 — Command Cut-over (terminal)

**Kanban project:** `command-cutover` · **Tier 3** · **Depends on:**
`builtin-commands` + `command-events` (and transitively everything below).

The big-bang milestone: move the frontend fully onto MCP and delete the old
YAML command architecture. The full-baseline e2e (all 62 commands through the
Command service) is the acceptance gate.

## Tasks

| Kanban id | Title | depends_on | Acceptance (one-liner) |
| --------- | ----- | ---------- | ---------------------- |
| `01KS36Y4NBDZMGH6QF963MD6FE` | Frontend: migrate all non-transport `invoke()` calls to MCP servers | entity, focus, window/app servers + useDispatchCommand | Real surface (verified): `get_entity`→`entity`, `spatial_*`→`focus`, `show_context_menu`→native render, `log_command` dropped; `dispatch_command` handled by the useDispatchCommand task. Dead handlers removed from `commands.rs`; `mcp_call`/`mcp_subscribe` kept; grep test enforces. |
| `01KS615SAVY176H2XWFC3ARR32` | Pre-flight: migrate non-plugin Rust consumers off `swissarmyhammer-commands` | store, ui-state, entity servers | Decouple `swissarmyhammer-entity`/`-views`/`-perspectives`/`-focus` from the crate's `Command`/`CommandContext`/`CommandError` + `OptionsRegistry`/`OptionsResolver`; relocate the `reconcile_post_undo_caches` convergence (do NOT lose it); workspace builds with the crate still present but unreferenced by these four. |
| `01KS36Z0FQYYS7TZ005K5G5CDG` | Cut-over: delete `swissarmyhammer-commands` crate + YAML files + loader | all builtin-commands + command-events + backends + store + engine + the pre-flight relocation | Old crate + 12 YAMLs gone (nav.yaml stays); no `use swissarmyhammer_commands::`; `cargo build/test --workspace` green; `full_baseline_e2e.rs` runs all 62 commands through the Command service. |

(The cut-over delete task's `depends_on` lists all 24 upstream tasks explicitly
so the board blocks it until the whole stack — including the pre-flight
consumer-migration task — is done.)

## Key decisions baked in

- **Cut-over, not transitional**: the Command service, plugins, backends,
  events, and frontend land together; the YAML loader + `swissarmyhammer-commands`
  crate are deleted in the same change set.
- Pre-flight relocations (must happen before deletion), via the dedicated
  pre-flight task (`01KS615SAVY176H2XWFC3ARR32`): `UIState` → `ui-state` server
  (plan 3); `WindowInfo`/`window_info` → `window` server (it IS used by
  `dynamic_sources.rs` + `menu.rs`, so this is required, not optional); the
  `Command`/`CommandContext`/`CommandError` traits + `OptionsRegistry`/
  `OptionsResolver` machinery consumed by `swissarmyhammer-entity`/`-views`/
  `-perspectives`/`-focus` migrated/relocated; and the `reconcile_post_undo_caches`
  convergence logic relocated to a surviving home (not deleted).
- The `full_baseline_e2e.rs` test reads the catalog (`plugins.yaml`, plan 4)
  and asserts every one of the 62 commands registers with the right metadata and
  executes to the same effect as the YAML version — the cut-over contract.

## Cross-check

`kanban list tasks --filter '$command-cutover'` → expect exactly these 3 tasks.
