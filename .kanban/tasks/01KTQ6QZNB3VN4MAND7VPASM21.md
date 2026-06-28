---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffa80
title: Drill-in / Escape broken in the third window (different board) after ownership-check fix — works in the two same-board windows
---
## What

LIVE BUG (user-observed, follow-on from `01KTQ3J9SDV7GBJ1XHZN1T2GRE`): with THREE windows open — two showing the SAME board, one showing a DIFFERENT board — drill-in / drill-out (Escape) now work correctly in the two same-board windows but DO NOT work in the third window.

## RESOLUTION (implemented — see log evidence + named root cause below)

**The label↔segment hypotheses were ruled out by the live log**: zero `ui_focus_owned_by_window` rejection warns in the whole repro window; every window's `focus-changed`/drill event showed `root_segment == window_label` exactly (`board-01krk5a157r86t9fb23xdy5bmk` etc. — lowercase-ULID labels, identity `asSegment`). The third window (`board-01krk5a157r86t9fb23xdy5bmk`, board `swissarmyhammer-plugin/.kanban`) navigated fine (nav.up/down/right logged and committed) but **never dispatched `nav.drillIn`/`nav.drillOut` at all** — total backend silence between 17:04:32 and 17:05:00 while the user pressed Enter/Escape. The two same-board windows (`board-01kt9qxe…`, `board-01ktecfxx…`, board `swissarmyhammer/.kanban`) drilled successfully.

**Named root cause — nondeterministic command-registry order + scope-blind first-id-wins key extraction:**
- `CommandRegistry::list()` (command service) iterates a `HashMap` — order explicitly "unspecified; callers that need a stable order must sort" — and `handle_list` did NOT sort.
- The webview builds its GLOBAL keybinding table via `extractKeymapBindings` (first-id-wins per key), ignoring each command's `scope` filter.
- TWO registry commands declare Enter: the global `nav.drillIn` and the scope-gated `ui.entity.startRename` (`scope: ["entity:perspective"]`).
- Each per-board plugin runtime owns its own registry HashMap instance → per-board random iteration order → per-board coin toss for Enter ownership. The third window's board runtime ordered `ui.entity.startRename` first, so its Enter resolved to the root-scope client-side `triggerStartRename()` (app-shell) — silently arming a perspective-rename editor and never reaching the backend (hence no log line). The armed rename input is an editable target, so Escape was swallowed by `isEditableTarget` as well. The two same-board windows share the other runtime where the toss favored `nav.drillIn`.

**Fix (both seams, no special cases):**
1. `extractKeymapBindings` (`apps/kanban-app/ui/src/lib/keybindings.ts`): scope-gated commands (non-empty `scope`) contribute NO global binding — their keys apply only via the focused-scope walk. Makes `nav.drillIn`/`nav.drillOut` the deterministic sole global owners of Enter/Escape.
2. `handle_list` (`crates/swissarmyhammer-command-service/src/service.rs`): sorts the `list command` response by id, honoring the registry's documented contract — every runtime now sees the identical sequence.

## Acceptance Criteria
- [ ] Three windows (two same board, one different board): drill-in and drill-out (Escape) work in ALL THREE windows, each committing focus only in its own window — NEEDS LIVE VERIFICATION after `tauri dev` rebuilds the Rust side (the sort fix is backend)
- [x] Root cause named: the exact comparison/normalization mismatch (or other cause) identified with log evidence — OTHER CAUSE, see Resolution
- [x] The label↔segment comparison is exact and consistent at every seam (single canonical form); no window label shape can be wrongly rejected as foreign — verified exact already (identity `asSegment`, exact `==` in Rust and JS) and pinned by the new `drill_in_accepts_own_provider_focus_for_ulid_window_label` invariant test using the live third-window label
- [x] Prior guards stay green: two-window-same-board guard, single-window drill guard, navigate/jump guards — `cargo nextest run -p swissarmyhammer-focus`: 121/121

## Tests
- [x] Regression test reproducing the third-window failure — `keybindings.test.ts` "Enter resolves to nav.drillIn regardless of registry order": adverse (third-window) order RED on old code (`Enter: ui.entity.startRename`), GREEN after; plus order-independence invariant. Backend: `list_filter.rs::list_returns_commands_in_deterministic_id_order` RED on old code (live HashMap scramble in failure output), GREEN after.
- [x] Invariant test: window-label segment accepted as own focus for the live lowercase-ULID label shape — `ui_geometry_provider.rs::drill_in_accepts_own_provider_focus_for_ulid_window_label` (host-driven drill, empty kernel slot, provider-pull + ownership check only)
- [x] `cargo nextest run -p swissarmyhammer-focus` (121/121) and `-p swissarmyhammer-command-service` (104/106 — only the two pre-existing out-of-scope failures from card 01KTPDTH772HSEV5F7R1DKYDNJ); touched vitest files green (keybindings 84/84; use-hotkeys, app-shell, app-shell.nav-commands, spatial-focus-context.responders, use-command-list all green; `tsc --noEmit` clean)
- [x] Review-blocker regression test — `badge-list-display.test.tsx` "tag pill keyboard untag" (2 tests): renders the REAL `BadgeListDisplay`, pulls the pill's registered `CommandScope` from the entity-focus registry, and proves `extractScopeBindings` + `createKeyHandler` dispatch `task.untag` on vim `x` / cua `Delete`. RED on pre-fix code (scope yielded only `Space: entity.inspect` — no untag keys), GREEN after adding the `keys` block to `useTagUntagCommands`.

Two PRE-EXISTING vitest failures (proved to fail identically with the pre-fix code) filed as follow-up card `01KTQ8KRJYX1DPHN76TZ654ZX2`: perspective-tab-bar.enter-rename test #3 (stale drill wire-shape expectation) and spatial-focus-context.test.tsx (missing `SERIALIZE_TO_IPC_FN` in its core mock).

## Constraints
- Do NOT run whole-workspace `cargo build`/`cargo clippy`/`cargo run` — `tauri dev` is hot-reloading; crate-scoped `cargo nextest run -p <crate>` only.
- Read the unified log yourself (`log show --predicate 'subsystem == "com.swissarmyhammer.kanban"'`) — never ask the user to check logs.
- Window identity comes from the fully-qualified scope chain; fix the comparison, do not add side fields or special cases.

## Workflow
- Used `/tdd` — failing tests first at both seams (vitest adverse-order extraction; Rust list-order), then the fixes. Second pass (review findings) also TDD: production-path RED test in `badge-list-display.test.tsx` before the `keys` fix.

## Review Findings (2026-06-09 17:55)

Review verified the full causal chain in code (all links hold), red-green probed the vitest adverse-order tests (4 fail with the scope-skip reverted, 84/84 restored), and re-ran the suites: focus 121/121, command-service 104/106 (only the two known out-of-scope e2e failures), `list_returns_commands_in_deterministic_id_order` passes against an 8-command fixture (1/8! false-pass odds — sound). Scoped Enter for rename remains functional in-scope via `ScopedPerspectiveTab`'s per-tab `CommandDef` (`keys: Enter`) picked up by `extractScopeBindings`. OS menu ordering is unaffected by the sort (`menu.rs` sorts by `(group, order)`). The prior ownership check and focus-changed filter are untouched and pinned by the new ULID-label invariant test. One real casualty found:

### Blockers
- [x] `apps/kanban-app/ui/src/lib/keybindings.ts` (`extractKeymapBindings` scope skip) — silently kills `task.untag`'s keyboard bindings. `task.untag` (`builtin/plugins/task-commands/index.ts`, `scope: ["entity:tag","entity:task"]`, `keys: { vim: "x", cua: "Delete" }`) is the OTHER scope-gated registry command with `keys`. Pre-fix, cua `Delete` was the deterministic SOLE global claimant (no collision) and dispatched `task.untag` to the backend with the focused scope chain — functional whenever a tag pill (mention-view `FocusScope` with `tag:<id>` moniker, task ancestor in chain) was spatially focused. Post-fix the registry keys are excluded from the global table AND nothing carries them in the scope walk: the badge-list `useTagUntagCommands` scope entry is `contextMenu: true` with NO `keys`, and `extractScopeBindings` reads only React scope commands' `keys`. Keyboard untag is now unreachable in all three keymaps; only the context-menu path survives. Fix the same way rename was fixed: add `keys: { vim: "x", cua: "Delete" }` to the tag pill's scope-level `task.untag` CommandDef (the exact `ScopedPerspectiveTab` pattern used for `ui.entity.startRename`'s Enter). Note pre-fix vim `x` was already the same coin-toss class (vs global `entity.cut`), and pre-fix cua `Delete` mis-fired `task.untag` everywhere (error dispatch on non-tag focus) — the scope-skip direction is right; the missing piece is the scope-side carrier. — **FIXED: `useTagUntagCommands` (`badge-list-display.tsx`) now carries `keys: { vim: "x", cua: "Delete" }` mirrored from the registry definition (read, not hardcoded blind — verified against `builtin/plugins/task-commands/index.ts`). Production-path regression guard added in `badge-list-display.test.tsx` ("tag pill keyboard untag", 2 tests): RED on pre-fix code, GREEN after. Innermost-scope-wins also resolves the old vim `x` coin toss vs `entity.cut` in the pill's favor.**

### Warnings
- [x] `apps/kanban-app/ui/src/hooks/use-hotkeys.test.tsx` — fixture drift: its `REGISTRY` models `task.untag` WITHOUT the `scope` field, so the suite stays green while the production-shaped (scope-gated) `task.untag` no longer binds `x`/`Delete` anywhere. The test's framing comment claims to pin `task.untag`'s hotkeys but it can no longer catch the blocker above. When fixing the blocker, refit this test to the real scoped shape (global table excludes it; scope walk binds it), or rename it to a neutral extraction-machinery test. — **FIXED: fixture refit to the production shape (`scope: ["entity:tag","entity:task"]` + keys). Suite now asserts: global table EXCLUDES the scope-gated keys (no leak, vim + cua), keys dispatch via the focused-scope path (`extractScopeBindings` over the pill-shaped CommandDef), keys do NOT fire without a focused scope, and keymap-switch rebind works for both the scope path and a stand-in global command (`app.demo`). 6/6 green.**

### Nits
- [x] `crates/swissarmyhammer-command-service/src/service.rs` — the id-sort also changes the command palette's unfiltered display order from per-runtime-random to id-alphabetical (palette renders `registryCommands` order; OS menu is unaffected). User-visible but strictly an improvement over the coin toss — no action needed beyond awareness. — **Acknowledged; no code change (per the finding itself).**

## Second-Pass Verification (2026-06-09)
- `badge-list-display.test.tsx`: 9/9 (2 new + 7 prior) — RED first (both new tests failed: scope had no untag keys), GREEN after the fix.
- `use-hotkeys.test.tsx`: 6/6.
- Combined touched vitest run (`keybindings.test.ts`, `badge-list-nav.test.tsx`, `badge-list-display.test.tsx`, `use-hotkeys.test.tsx`): 106/106; `npx tsc --noEmit` clean.
- `cargo nextest run -p swissarmyhammer-command-service`: 104/106 — only the two known out-of-scope e2e failures (card 01KTPDTH772HSEV5F7R1DKYDNJ), unchanged.