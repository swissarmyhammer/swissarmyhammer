---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffe80
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

## Review Findings (2026-06-10 06:45)

Verified at HEAD: the keybinding metadata change is INTENTIONAL and CORRECT — `app.dismiss` (builtin/plugins/app-shell-commands/commands/app.ts) carries no `keys`, `ui.inspector.close` (builtin/plugins/ui-commands/index.ts) carries `{ vim: "q" }` only, `ui.inspector.close_all` keeps `{ cua: "Mod+Escape", vim: "Q" }`, and the `STATIC_GLOBAL_COMMANDS` `app.dismiss` entry is key-less; all four sites have comments citing this card. Escape→nav.drillOut survived 541e85ce3 (keybindings.test.ts "Escape resolves to nav.drillOut" suite green; scoped vitest on keybindings.test.ts + app-shell.test.tsx: 101/101 pass). builtin_nav_commands_e2e 3/3 pass including sections 3d (palette dismiss fall-through) and 3e (inspector pop at layer-root edge) — drill-out intent survives the 4a3a2c780 server-resolved-source/moved-flag refinement. `cargo nextest run -p swissarmyhammer-command-service`: 104/106 pass; the ONLY 2 failures are this card's collateral — e2e metadata-fidelity expectations never updated to the intended new key state. Do NOT change the plugin metadata; fix the test expectations (declared == actual).

### Blockers
- [x] `crates/swissarmyhammer-command-service/tests/integration/builtin_app_shell_commands_e2e.rs:594` — `assert_app_dismiss` still asserts the OLD keys `{ "vim": "Escape", "cua": "Escape", "emacs": "Escape" }`; actual registered metadata at HEAD is no `keys` (Null). Test fails: `app_shell_commands_plugin_registers_and_executes`. Fix: assert no keys (the file already has an `assert_no_keys` helper — reuse it) and update the stale doc comment above the fn ("app.yaml: keys vim:Escape / cua:Escape / emacs:Escape") to state that `app.dismiss` is intentionally unbound from Escape per this card (Escape is owned by `nav.drillOut`). — FIXED 2026-06-10: `assert_app_dismiss` now uses `assert_no_keys`; doc comment rewritten to cite this card and `nav.drillOut` ownership. Test green.
- [x] `crates/swissarmyhammer-command-service/tests/integration/builtin_ui_commands_e2e.rs:724` — `assert_inspector_close` still asserts the OLD keys `{ "cua": "Escape", "vim": "q" }`; actual registered metadata at HEAD is `{ "vim": "q" }` only. Test fails: `ui_commands_plugin_registers_and_executes`. Fix: assert `json!({ "vim": "q" })` and update the stale doc comment ("ui.yaml: keys cua:Escape / vim:q") to note cua:Escape was removed per this card (inspector Escape-close flows through `nav.drillOut` → `dismiss ui`; vim `q` remains a direct close). — FIXED 2026-06-10: asserts `json!({ "vim": "q" })`; doc comment rewritten per the finding. Test green.

### Nits
- [x] `apps/kanban-app/ui/src/components/app-shell.tsx:361-375` — the `buildDynamicGlobalCommands` docblock still says drill commands must come first "so they shadow the static `app.dismiss: Escape` binding" / "to claim Escape away from `app.dismiss`". That static Escape binding no longer exists (this card stripped it); the ordering rationale described is stale. Reword to the current reality (e.g. ordering keeps drill commands first in `extractScopeBindings`'s first-key-wins walk, with no app.dismiss Escape to contend with) or drop the obsolete shadow justification. — FIXED 2026-06-10: docblock reworded — ordering rationale is now the first-key-wins walk in `extractScopeBindings` guaranteeing `nav.drillOut: Escape` reaches the scope map first, with an explicit note that the static `app.dismiss` entry is key-less (comment-only change).

## Fix Verification (2026-06-10)
`cargo nextest run -p swissarmyhammer-command-service`: 106 tests run, 106 passed, 0 skipped (was 104/106). Both `app_shell_commands_plugin_registers_and_executes` and `ui_commands_plugin_registers_and_executes` confirmed PASS individually. No plugin metadata touched; app-shell.tsx change is comment-only (no vitest/tsc needed).