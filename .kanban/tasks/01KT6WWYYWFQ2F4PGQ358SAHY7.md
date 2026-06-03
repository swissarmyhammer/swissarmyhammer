---
assignees:
- claude-code
position_column: todo
position_ordinal: ad80
project: command-cutover
title: Migrate ai.yaml AI-panel commands off the YAML command model (last non-nav YAML command source)
---
DISCOVERED finishing the big-bang cut-over (01KS36Z0). That card deleted the 12 enumerated command YAMLs + the `swissarmyhammer-commands` crate. But one YAML command source remains beyond the explicitly-exempt `swissarmyhammer-focus/builtin/commands/nav.yaml`:

- `crates/swissarmyhammer-kanban/builtin/commands/ai.yaml` — AI-panel window-layer commands (open/close the right-docked AiPanelContainer, drive the ACP conversation). Per its own header they're declared in YAML "so they surface in the command palette, carry keybinding hints, and pass the YAML <-> Rust completeness guard"; behaviour is webview-side (React/ACP, per-board). This postdates the cut-over plan (AI-panel project), so it wasn't in the 12-YAML scope.

Per "everything is MCP / commands live in plugin code, not YAML", these should move to the builtin command-plugin model like the other 7 plugins (a small `ai-commands` builtin plugin registering via `registerCommands`, with `execute` callbacks that drive the webview AI panel — or routing to an `ai` server if/when one exists). Coordinate with the ai-panel project.

## Work
- Port `ai.yaml`'s commands to a builtin TS command plugin under `builtin/plugins/` (or fold into an existing one if a natural home exists), preserving palette/keybinding metadata 1:1.
- Remove `ai.yaml` and whatever loads it; update the YAML<->Rust completeness guard so it no longer expects it.
- Clean two stale doc-comment references to the deleted crate in `crates/swissarmyhammer-kanban/src/commands_core/macros.rs:37,69` (`/// use swissarmyhammer_commands::...`) — point them at the current home of `compose_registry`/`compose_yaml_sources`.

## Acceptance
- No YAML under `crates/swissarmyhammer-*/builtin/commands/` except `swissarmyhammer-focus/.../nav.yaml`.
- AI-panel commands still appear in the palette with their keybindings and still drive the panel.
- Stale `swissarmyhammer_commands` doc references gone.
- `cargo build --workspace` + `npm test` (kanban-app ui) green; completeness guard passes.

Note: the cut-over card 01KS36Z0 is otherwise complete (crate + 12 YAMLs deleted, full_baseline_e2e + no_stale_imports green); this is the residual YAML surface that appeared after the plan was written.