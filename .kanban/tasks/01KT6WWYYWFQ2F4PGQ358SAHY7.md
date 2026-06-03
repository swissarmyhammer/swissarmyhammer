---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffe780
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

## Review Findings (2026-06-03 19:05)

### Warnings
- [x] 1. `ai.cancel` palette gating gone — registry-driven palette shows "Stop AI Generation" enabled even when idle. FEASIBILITY: confirmed no clean synchronous gate exists in the current SDK. `available` is contracted SYNCHRONOUS (command-service.md:133); the streaming flag lives webview-side in `ai/commands.ts`'s module bus; `CommandContext` carries only `scope_chain`/`target`/`args` (no streaming flag); the event-driven cached-flag pattern needs the SDK `on`/`subscribe` API, which is RESERVED/inert (`sdk/plugin.ts` `reservedHandler` returns a no-op). DECISION: took the FALLBACK path — kept `ai.cancel` always-available at the registry layer, (a) fixed the stale doc in `apps/kanban-app/ui/src/ai/commands.ts` to describe the real gate (frontend `buildAiCommands available: streaming` governs the React-scope palette + keybinding), and (b) filed follow-up card `01KT7DB01HTR9SNRRG145F009P` in command-cutover ("Gate ai.cancel in the palette via event-driven cached availability (needs SDK subscribe API)"), referenced from the `ai-commands/index.ts` header.
- [x] 2. `apps/kanban-app/ui/src/ai/commands.ts` — stale doc claiming the streaming flag is "mirrored into the backend UIState so commands_for_scope keeps the palette entry hidden when idle". Rewrote the doc: the gate is frontend-owned (`buildAiCommands available: streaming` → React-scope palette + keybinding); the registry palette has no `available` (no synchronous webview view); added a historical note that the old `UIState.ai_streaming` / `AiCancelCmd::available()` path was retired in this migration.

### Nits
- [x] 3. `crates/swissarmyhammer-kanban/src/commands/mod.rs` `test_no_builtin_yaml_command_sources_remain` — added a comment making the tripwire value explicit: the realistic regression it guards is that the two `builtin_yaml_sources()` embedding points (`commands_core::builtin_yaml_sources` + `crate::builtin_yaml_sources`) stay empty; if a future change re-embeds a YAML command source the test fails.
- [x] 4. `builtin/plugins/ai-commands/index.ts` — replaced the hardcoded "5" in the `log.info` with a count derived from the registration array (`commands.length` + the ids joined from the array), so the message can never drift from the array.