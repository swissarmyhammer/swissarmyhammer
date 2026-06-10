---
assignees:
- claude-code
position_column: review
position_ordinal: '8680'
project: ui-command-cleanup
title: Escape = nav.drillOut with contextual dismiss-at-root (untangle app.dismiss)
---
## What
Make Escape = drill out (nav.drillOut), symmetric with Enter = drill in (which WORKS). Owner decision: "Escape needs to be drill out, and drill out needs to know what to do. app.dismiss is trash."

## RESOLUTION (implemented 2026-06-09)
The bug was purely a keybinding collision — the dismiss-at-root logic was already correct. `nav.drillOut` (Escape) drills out one focus level via the kernel and, at a layer-root edge, falls through to `ui_state dismiss ui` (layered close: palette → inspector). The jump overlay self-handles Escape in its own document-capture listener (`useKeyMatcher`, stopImmediatePropagation), so it never depended on `app.dismiss` for Escape. The competing Escape bindings shadowed `nav.drillOut`:
- root scope `STATIC_GLOBAL_COMMANDS app.dismiss: Escape` (scope wins over global) — the decisive shadow per the trace.
- registry `app.dismiss` (app.ts) Escape and `ui.inspector.close` (ui-commands) cua:Escape.

### Changes (file:symbol)
- `apps/kanban-app/ui/src/components/app-shell.tsx` `STATIC_GLOBAL_COMMANDS` — `app.dismiss` kept as a command id but stripped of its `keys` (no Escape). Removes the scope-level shadow; command stays discoverable + dispatchable (inspector backdrop click, quick-capture).
- `builtin/plugins/app-shell-commands/commands/app.ts` `app.dismiss` — removed `keys` (was Escape). Command + `ui_state dismiss ui` routing unchanged.
- `builtin/plugins/ui-commands/index.ts` `ui.inspector.close` — `keys` now `{ vim: "q" }` only (removed cua:Escape). x-button onClick + `ui.inspector.close_all` (Mod+Escape) untouched. Inspector Escape-close now flows through `nav.drillOut` → `dismiss ui` (pops the topmost inspector).
- `builtin/plugins/nav-commands/index.ts` `nav.drillOut` — UNCHANGED; already the desired drill-out + dismiss-fallthrough. Now the sole Escape owner.

`app.dismiss` retired as an Escape binding but kept as a command (per-surface dismiss hook preserved: jump sentinel shadow + backdrop/quick-capture dispatch).

## Acceptance Criteria
- [x] Escape drills out one focus level when inside a drillable scope (kernel commits focus to parent + emits focus-changed).
- [x] Escape at a layer root closes the active overlay/inspector/palette contextually (host: `dismiss ui` layered close; jump: self-handled capture) — no regressions.
- [x] `nav.drillOut` is the resolved Escape command (verified via the keybinding path), not `app.dismiss`.
- [x] x button still closes the inspector; Mod+Escape still closes-all; vim `q` still closes inspector.
- [x] No leftover frontend `app.dismiss` scope binding intercepting Escape.

## Tests (automated, RED→GREEN proven)
- [x] Keybinding unit test (`apps/kanban-app/ui/src/lib/keybindings.test.ts`): "Escape resolves to nav.drillOut (production registry + scope wiring)" — global registry layer + root scope both resolve Escape→nav.drillOut.
- [x] AppShell integration RED→GREEN (`apps/kanban-app/ui/src/components/app-shell.test.tsx`): "Escape resolves to nav.drillOut, not app.dismiss, from a focused card scope" (was RED: dispatched app.dismiss; now nav.drillOut). Updated "keyboard dispatch includes scopeChain…" + "keybinding handler resolves commands from focused scope" (now uses a scope dialog.cancel Escape claim).
- [x] Kernel test (`crates/swissarmyhammer-focus/tests/integration/ui_geometry_provider.rs`): "drill_out_with_parent_zone_commits_focus_to_parent_and_emits_event" — drill-out from a scope WITH a parent_zone commits focus to the parent (kernel slot) + emits.
- [x] Bus/overlay / e2e (`crates/swissarmyhammer-command-service/tests/integration/builtin_nav_commands_e2e.rs`): added section 3e — nav.drillOut at a layer-root edge pops the open inspector (inspector Escape-close path). Existing 3d (palette close) stays green.
- [x] Regression: jump-overlay + palette Escape-close tests unchanged; jump-overlay/inspectors-container pre-existing failures verified identical pre/post via git stash baseline (no NEW failures introduced).

NOTE: UI changes hot-reload; the Rust/plugin changes (app.ts, ui-commands, nav unchanged) require the user's `tauri dev` rebuild/restart to verify the live overlay-dismiss behavior.

## Workflow
- Used `/tdd`. Depends on the webview handler bus (Card B, landed). Relates to Card I (STATIC_GLOBAL_COMMANDS) and nav-regression 01KTESYQ49JYJB2YT1WXYKK0W4. #bug