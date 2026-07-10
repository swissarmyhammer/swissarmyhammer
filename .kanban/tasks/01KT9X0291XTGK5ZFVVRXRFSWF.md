---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kvvrr2ehwcbea205qnhcts0g
  text: |-
    Picked up. Research findings (corrected stale refs):

    - Bridge constructor `McpNotification::ui_state_changed(window, key, value)` exists in `crates/swissarmyhammer-plugin/src/notify.rs` (Plane 4) — confirmed NO publisher today.
    - Reference pattern to mirror: `AiStreamingChanged` `#[notification]` struct + `ai_streaming_notification()` helper in `crates/swissarmyhammer-ui-state/src/operations.rs`, published via `bridge.publish(...)` in `apps/kanban-app/src/ai/models.rs` (`publish_ai_streaming`). Coverage-guard pattern from `swissarmyhammer-command-service/src/txn.rs` (`declared_methods()` == raised).
    - Direct emitter `emit_ui_state_change_if_needed` is in `apps/kanban-app/src/commands.rs` — but its payload is `{ kind, state: <FULL UiState snapshot> }`, NOT thin key/value. `ui_state_change_kind()` maps `UiStateChange` enum variants to kind strings (scope_chain/palette_open/keymap_mode/inspector_stack/active_view/active_perspective/app_mode/inspector_width/perspective_switch). Board kinds (board_switch/board_close) come from board side-effects, not UiStateChange.
    - watcher.rs `recompute_and_emit_perspective_filters` emits `{ kind: perspective_switch, state }` per window via `window.emit("ui-state-changed", ...)`.
    - Frontend: `apps/kanban-app/ui/src/lib/ui-state-context.tsx` `UIStateProvider` listens `ui-state-changed`, replaces its WHOLE snapshot from `event.payload.state`, suppressing `scope_chain`. The bridge-forwarded path already has a `subscribeUiStateChanged()` helper + `UiStateChanged {window?,key,value}` interface in `apps/kanban-app/ui/src/lib/mcp-notifications.ts` — but it is currently UNUSED.
    - Bridge→Tauri pump (`spawn_window_forwarder` in commands.rs) already forwards every bridge notification to the window as a Tauri event named by `method`, emitting `params`. So once published, frontend can `listen("notifications/ui_state/changed")`.

    Open design question: provider relies on full snapshot. Investigating whether thin payload suffices or snapshot must ride along.
  timestamp: 2026-06-24T02:53:30.449358+00:00
- actor: claude-code
  id: 01kvvrvrjm0pst95w5jc72jh1z
  text: |-
    Design decision resolved.

    The direct `ui-state-changed` event carries `{ kind, state: <full per-window UiState snapshot> }`. The thin `{window,key,value}` form would DROP behavior: PaletteOpen(bool) loses `palette_mode` (read by app-shell.tsx to render palette mode), and compound kinds (perspective_switch carries id+filtered_task_ids, inspector_width carries window+width) don't fit a single `value`.

    Resolution: declare `#[notification] UiStateChanged { kind, state }` in ui-state operations.rs (struct=payload, mirroring AiStreamingChanged). Publish via `McpNotification::from_declared` carrying the SAME `{kind, state}` the direct event carried — the snapshot is already in hand at publish time, so this is NOT an enrichment re-fetch round-trip (the thin-events principle's actual concern). This preserves all frontend behavior and keeps declared==published guarded.

    Frontend swaps its `listen("ui-state-changed")` to the bridge-forwarded `notifications/ui_state/changed`, keeping the `{kind, state}` access. The unused `subscribeUiStateChanged()` helper's `UiStateChanged {window,key,value}` interface in mcp-notifications.ts will be reshaped to `{kind, state}` to match. Direct emits removed from commands.rs + watcher.rs.

    board_switch/board_close kinds exist only in frontend types/tests; NO Rust path emits them today (ui_state_change_kind only maps UiStateChange variants). Out of scope to remove — leaving the type union intact.
  timestamp: 2026-06-24T02:55:31.412374+00:00
- actor: claude-code
  id: 01kvvv51m62knzg335fzpekzkh
  text: |-
    Implementation landed (TDD throughout, RED verified before each GREEN).

    Rust (swissarmyhammer-ui-state):
    - state.rs: added `UiStateChange::kind()` -> &'static str (single source of truth for the wire discriminator; one arm per variant). Test `ui_state_change_kind_maps_every_variant`.
    - operations.rs: declared `#[notification] UiStateChanged { kind, state }` (method notifications/ui_state/changed, event defaults to "changed"), registered in UI_STATE_NOTIFICATIONS, added `ui_state_changed_notification(kind, state)` helper via from_declared (struct=payload). Tests: declares method/event, payload serialization, _meta advertises "changed", coverage-guard (declared==raised), payload-builds, and REAL-PIPELINE `mutation_publishes_ui_state_changed_on_the_bridge` (real UiState mutation -> kind()+to_json() -> notification -> real NotificationBridge -> live subscriber receives {kind,state}+provenance).
    - lib.rs: re-export UiStateChanged + ui_state_changed_notification.
    - tests/integration/meta_snapshot.rs: added production-path `ui_state_tool_meta_advertises_changed_notification` (drives real UiStateServer::list_tools, asserts .on("changed") resolves).

    App (kanban-app):
    - commands.rs: replaced `emit_ui_state_change_if_needed` (direct emit_to ui-state-changed) with async `publish_ui_state_change_if_needed` -> `publish_ui_state_changed`/`publish_ui_state_changed_to` which resolve each window's bridge (dedup on BindKey) and publish the declared notification. `ui_state_change_kind` now delegates to `UiStateChange::kind()`. Existing ui_state_change_kind contract tests stay green (regression guard).
    - watcher.rs: `recompute_and_emit_perspective_filters` now publishes on the bridge via publish_ui_state_changed_to for changed windows (was window.emit ui-state-changed). Removed unused Emitter import.

    Frontend:
    - ui-state-context.tsx: provider swapped from listen("ui-state-changed") to subscribeUiStateChanged (notifications/ui_state/changed), same {kind,state} handling.
    - mcp-notifications.ts: reshaped UiStateChanged interface {window,key,value} -> {kind,state} to match declared payload.
    - no-tauri-change-listeners.node.test.ts: added "ui-state-changed" to forbidden list (guards no direct emit returns).
    - Updated 4 browser tests to fire notifications/ui_state/changed, plus stale-comment cleanup.

    Verification:
    - cargo nextest -p swissarmyhammer-ui-state: 151/151 pass (lib) + 22/22 integration.
    - cargo nextest -p kanban-app -E 'not test(ai_panel_e2e)': 185/185 pass.
    - npx tsc --noEmit clean; affected vitest files all green (ui-state-context 8, mcp-notifications guard, inspector x2, left-nav, window-container, app-shell, command-palette, rust-engine, inspectors-container, inspector-resize, views-container).
    - cargo fmt + clippy --tests clean on my touched files (commands.rs/watcher.rs/ui-state).

    PRE-EXISTING failures (NOT mine, verified by git stash on commit 7a34f8496): ai_panel_e2e qwen 2 timeouts (GPU/model), swissarmyhammer-plugin file_notes_e2e + example_layering_e2e (filesystem relative-path/CWD isolation). I did not touch swissarmyhammer-plugin source.
  timestamp: 2026-06-24T03:35:32.742764+00:00
- actor: claude-code
  id: 01kvvwft2366kjf4td5ta13vna
  text: |-
    Worked the two IN-SCOPE review findings (per reviewer triage); left all pre-existing/out-of-scope findings deferred (annotated `[DEFERRED ...]` in the checklist, still unchecked).

    Finding 1 — single source of truth for the bridge event names in tests. The constants `UI_STATE_CHANGED_EVENT` and `STORE_CHANGED_EVENT` already exist and are exported from `apps/kanban-app/ui/src/lib/mcp-notifications.ts`, so no new export was needed — I imported and used them in the 5 test files that hardcoded the raw literal:
    - src/lib/ui-state-context.test.tsx — import `{ UI_STATE_CHANGED_EVENT }` from `./mcp-notifications`; replaced the `expect(...).toContain(...)` assertion + 2 `callbacks.get(...)` calls.
    - src/components/inspector.close-restores-focus.browser.test.tsx — import from `@/lib/mcp-notifications`; replaced `listeners.get(...)`.
    - src/components/inspector.repeat-open-focus.browser.test.tsx — import; replaced `listeners.get(...)`.
    - src/components/left-nav.view-switch.browser.test.tsx — import; replaced `emitTauriEvent(...)`.
    - src/components/window-container.test.tsx — import `{ STORE_CHANGED_EVENT, UI_STATE_CHANGED_EVENT }`; replaced the 2 store assertions/emit + 1 ui_state emit. (The remaining raw literal in rust-engine-container.test.tsx was NOT flagged and is out of the in-scope file set; it keeps its own local const — left untouched.)

    Finding 2 — test `kind` values now match the real backend discriminator. The authoritative value-space is `UiStateChange::kind()` in crates/swissarmyhammer-ui-state/src/state.rs (scope_chain, active_view, active_perspective, palette_open, keymap_mode, inspector_stack, app_mode, inspector_width, perspective_switch — there is NO `InspectorClosed`/`InspectorOpened`). Corrected:
    - inspector.close-restores-focus.browser.test.tsx: `kind: "InspectorClosed"` → `"inspector_stack"`.
    - inspector.repeat-open-focus.browser.test.tsx: all 3 `emitUiStateChanged("InspectorOpened"/"InspectorClosed")` call sites → `"inspector_stack"`.

    Verification (run in apps/kanban-app/ui):
    - `npx tsc --noEmit` → exit 0, clean.
    - `npm test -- src/lib/ui-state-context.test.tsx` → 8/8 pass.
    - `npm test -- src/components/inspector.close-restores-focus.browser.test.tsx src/components/inspector.repeat-open-focus.browser.test.tsx src/components/left-nav.view-switch.browser.test.tsx src/components/window-container.test.tsx` → 4 files / 16 tests pass.
    No Rust source touched (only read state.rs for the authoritative kind set), so no cargo nextest/fmt needed.

    Task stays in `review`.
  timestamp: 2026-06-24T03:58:54.019261+00:00
- actor: wballard
  id: 01kvvwh3v2fbt360xqb4qwha9v
  text: 'Review resolution — task contract certified CLEAN by reviewer (all 5 load-bearing items pass: correct per-window bridge publish, declared==raised with a real drift-catching coverage guard, complete frontend swap + reintroduction guard, genuine real-pipeline test, conventions honored). 2 in-scope test-hygiene findings fixed: (1) replaced raw `''notifications/ui_state/changed''` literals across 5 frontend test files with the existing exported `UI_STATE_CHANGED_EVENT`/`STORE_CHANGED_EVENT` constants from lib/mcp-notifications.ts; (2) corrected fictional `kind` values `InspectorOpened`/`InspectorClosed` → real discriminator `inspector_stack` (authoritative: `UiStateChange::kind()` in ui-state/src/state.rs). Verified: `npx tsc --noEmit` clean; affected vitest files green (ui-state-context 8/8 + 4 files/16 tests). Out-of-scope/pre-existing findings (InspectorsContainer/InspectorPanel/ViewsProvider length, `m` abbrev, missing doc comment, hardcoded test timeouts, Tauri-mock dup) deferred — they predate this diff. Moving to done.'
  timestamp: 2026-06-24T03:59:36.802757+00:00
depends_on:
- 01KT9JV0N1RF0MMEBRZV4A6F3J
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffe180
project: command-events
title: Route UI-state changes onto the bridge (ui_state/changed)
---
Make ephemeral UI-state changes observable on the bridge and consumed by the frontend from the bridge.

Current state: the app emits the direct Tauri `ui-state-changed` event (apps/kanban-app/src/commands.rs:1889 `emit_ui_state_change_if_needed`; watcher.rs:877 perspective-filter recompute) with `kind` discriminators (scope_chain, palette_open, keymap_mode, inspector_stack, active_view, active_perspective, app_mode, inspector_width, perspective_switch, board_switch, board_close). The bridge plane `notifications/ui_state/changed` (notify.rs:296 constructor) has NO publisher.

## Work
- Publish `notifications/ui_state/changed` on the bridge from the ui-state service when UI state mutates (carry the same `kind`/window/key/value payload).
- Swap the frontend `ui-state-changed` listener to the bridge-forwarded `notifications/ui_state/changed`; remove the direct emits (commands.rs:1889, watcher.rs:877) once swapped.
- Declare on the ui-state service tool (swissarmyhammer-ui-state/src/service.rs:106) via #[notification] struct=payload. The `kind` set should be captured (enum or documented value space) so the declared params describe it.
- Coverage guard.

## Acceptance
A plugin can `this.ui_state.on("changed", cb)`; the frontend reads UI-state changes from the bridge; no direct `ui-state-changed` emit remains; declared == published.

## Review Findings (2026-06-23 21:39)

### Blockers
- [ ] [DEFERRED — pre-existing/out-of-scope per reviewer triage: Tauri-mock duplication is borderline test-helper hygiene, not required for the task contract] `apps/kanban-app/ui/src/components/inspector.close-restores-focus.browser.test.tsx:18` — Verbatim duplication of Tauri API mock setup (lines 18–36) with inspector.repeat-open-focus.browser.test.tsx (lines 32–50). Both define identical `mockInvoke`, `mockListen`, and `listeners` hoisted mocks, including the `ListenCallback` type. This violates DRY and creates a maintenance burden if the mock contract changes. Extract the mock setup into a shared test utility function in a common test helpers module (e.g., `apps/kanban-app/ui/src/test-helpers/tauri-mocks.ts`), then import and use it in both test files.

### Warnings
- [x] `apps/kanban-app/ui/src/components/inspector.close-restores-focus.browser.test.tsx:203` — FIXED: now imports `UI_STATE_CHANGED_EVENT` from `@/lib/mcp-notifications` and uses it in `listeners.get(UI_STATE_CHANGED_EVENT)`.
- [x] `apps/kanban-app/ui/src/components/inspector.repeat-open-focus.browser.test.tsx:222` — FIXED: now imports and uses `UI_STATE_CHANGED_EVENT` instead of the hardcoded string.
- [ ] [DEFERRED — pre-existing/out-of-scope: this diff changed only doc-comment strings in inspectors-container.tsx; the `m` naming predates this task] `apps/kanban-app/ui/src/components/inspectors-container.tsx:79` — Variable `m` is an abbreviation for 'moniker'. The naming rule requires full words only (error not e, callback not cb, etc.). Rename `m` to `moniker` for clarity and consistency with the style guide.
- [ ] [DEFERRED — pre-existing/out-of-scope: pre-existing component structure, not introduced here] `apps/kanban-app/ui/src/components/inspectors-container.tsx:110` — InspectorsContainer spans ~85 lines of code (excluding comments/blanks), exceeding the 50-line threshold. While React components with many hooks are common, this function combines multiple concerns: panel state sync, width management, drag tracking, and event dispatch setup. Extracting resize logic and event handlers into separate custom hooks would improve readability and testability. Extract resize handling (lines 135-154) into a custom hook like `useInspectorResize()`, and extract event handlers (lines 157-184) into separate memoized callbacks or a custom hook to reduce the component's cognitive load.
- [ ] [DEFERRED — pre-existing/out-of-scope: pre-existing component structure, not introduced here] `apps/kanban-app/ui/src/components/inspectors-container.tsx:204` — InspectorPanel spans ~57 lines of code, exceeding the 50-line threshold. The function mixes entity fetching logic (with dedup tracking) and JSX rendering. The fetch dedup pattern could be extracted into a custom hook to separate concerns. Extract the entity fetching with dedup logic (lines 210-253) into a custom hook like `useFetchEntity(entityType, entityId)` that returns `{ resolved, fetchError }`, leaving InspectorPanel to focus on rendering.
- [x] `apps/kanban-app/ui/src/components/left-nav.view-switch.browser.test.tsx:119` — FIXED: now imports `UI_STATE_CHANGED_EVENT` and uses `emitTauriEvent(UI_STATE_CHANGED_EVENT, ...)`.
- [ ] [DEFERRED — pre-existing/out-of-scope: `createMockListen` factory refactor is borderline test-helper hygiene, not the task contract] `apps/kanban-app/ui/src/components/window-container.test.tsx:20` — Near-verbatim duplication: `mockListen` (lines 26–38) and `mockWindowListen` (lines 39–51) are identical except for the map they operate on (`listeners` vs `windowListeners`). This is one function with a parameter, not two separate implementations. Extract a factory function `createMockListen(mapInstance: Map<string, ListenCallback[]>)` that returns the listener implementation parameterized on the map. Replace both hoisted mock definitions with calls to this factory.
- [x] `apps/kanban-app/ui/src/components/window-container.test.tsx:246` — FIXED: now imports `UI_STATE_CHANGED_EVENT` and `STORE_CHANGED_EVENT` from `@/lib/mcp-notifications` and uses them in place of the hardcoded strings in assertions/emits.
- [x] `apps/kanban-app/ui/src/lib/ui-state-context.test.tsx:130` — FIXED: now imports `UI_STATE_CHANGED_EVENT` from `./mcp-notifications` and uses it in the `expect(events).toContain(...)` assertion and the `callbacks.get(...)` calls.
- [ ] [DEFERRED — pre-existing/out-of-scope: this diff changed only one doc-comment string in views-context.tsx; the function length predates this task] `apps/kanban-app/ui/src/lib/views-context.tsx:20` — ViewsProvider spans ~60 lines of code, exceeding the 50-line threshold. The function combines view list management, active view selection dispatch, and two subscription effects (lifecycle and store changes). The subscription logic could be extracted. Extract the store-change subscription (lines 54-74) into a custom hook like `useRefreshViewsOnStoreChange(refresh)` to isolate the effect and improve component focus.

### Nits
- [x] `apps/kanban-app/ui/src/components/inspector.close-restores-focus.browser.test.tsx:200` — FIXED: `kind: 'InspectorClosed'` changed to `kind: 'inspector_stack'` (the real backend discriminator per `UiStateChange::kind()` in crates/swissarmyhammer-ui-state/src/state.rs).
- [ ] [DEFERRED — pre-existing/out-of-scope: hardcoded test timeout is pre-existing scaffolding, not introduced here] `apps/kanban-app/ui/src/components/inspector.close-restores-focus.browser.test.tsx:242` — Hardcoded timeout value `50` configures test behavior and should be extracted to a named constant for clarity and maintainability. Extract `50` to a named constant like `const TEST_FLUSH_DELAY_MS = 50;` at module scope, then use `setTimeout(r, TEST_FLUSH_DELAY_MS)`.
- [x] `apps/kanban-app/ui/src/components/inspector.repeat-open-focus.browser.test.tsx:222` — FIXED: the three `emitUiStateChanged("InspectorOpened"/"InspectorClosed")` call sites now pass `"inspector_stack"`, the real backend discriminator.
- [ ] [DEFERRED — pre-existing/out-of-scope: hardcoded test timeout is pre-existing scaffolding, not introduced here] `apps/kanban-app/ui/src/components/inspector.repeat-open-focus.browser.test.tsx:427` — Hardcoded timeout value `1500` configures test behavior and should be extracted to a named constant for consistency and maintainability across multiple waitFor calls. Extract `1500` to a named constant like `const WAITFOR_TIMEOUT_MS = 1500;` at module scope, then use `{ timeout: WAITFOR_TIMEOUT_MS }`.
- [ ] [DEFERRED — pre-existing/out-of-scope: this diff changed only one doc-comment string in views-context.tsx; missing doc comment predates this task] `apps/kanban-app/ui/src/lib/views-context.tsx:29` — Public exported function `ViewsProvider` lacks a documentation comment. Context providers are key infrastructure components that should document what context they manage, what data they provide to consumers, and their role in the app. Add a documentation comment explaining the provider's purpose, e.g.: `/** Provider that manages the views list and active view selection for a window. Subscribes to store changes and dispatches view.set through the command system. Must wrap ViewsProvider in a UIStateProvider. */`.

### In-scope work completed (2026-06-23)

Addressed ONLY the two in-scope finding families per reviewer triage:
1. Event-name literal `'notifications/ui_state/changed'` (and `'notifications/store/changed'` in window-container) replaced by the shared exported constant `UI_STATE_CHANGED_EVENT` / `STORE_CHANGED_EVENT` (already exported from `apps/kanban-app/ui/src/lib/mcp-notifications.ts`) across the 5 test files: ui-state-context.test.tsx, inspector.close-restores-focus.browser.test.tsx, inspector.repeat-open-focus.browser.test.tsx, left-nav.view-switch.browser.test.tsx, window-container.test.tsx.
2. Test `kind` values `InspectorClosed`/`InspectorOpened` corrected to `inspector_stack` (the authoritative backend discriminator from `UiStateChange::kind()`).

All other findings are DEFERRED as pre-existing/out-of-scope per the reviewer triage (InspectorsContainer/InspectorPanel/ViewsProvider length, `m` abbreviation, missing doc comments, hardcoded test timeouts, Tauri-mock/createMockListen duplication).

Verification: `npx tsc --noEmit` clean; affected vitest files green (ui-state-context 8/8; inspector.close-restores-focus + inspector.repeat-open-focus + left-nav.view-switch + window-container 16/16 together). No Rust touched.

### Reviewer triage (scope of THIS task's diff vs pre-existing noise)

The engine reviewed the full working tree. I verified each flagged file against `git diff HEAD` for this task. Triage:

IN-SCOPE (introduced or directly caused by this diff — worth addressing):
- Hardcoded `'notifications/ui_state/changed'` literal repeated across 5+ test files (the 6 "rule of three" warnings/nits on event-name strings, plus the `InspectorClosed`/`InspectorOpened` test-kind nits). This task introduced the new bridge event into these tests, so the literal proliferation is a direct consequence. Low-severity test hygiene; recommend exporting a `UI_STATE_CHANGED_EVENT` constant from `mcp-notifications.ts` and importing it in tests. The `InspectorClosed`/`InspectorOpened` test kinds also do not match the real backend discriminator set — worth aligning to `inspector_stack` so the new tests exercise the real contract.
- The Blocker (Tauri-mock duplication across the two `inspector.*.browser.test.tsx` files) and the `window-container.test.tsx` `createMockListen` factory warning are test-helper hygiene in files this task edited. Borderline in-scope; reasonable to extract a shared mock helper but not required for the contract.

OUT-OF-SCOPE / PRE-EXISTING (these files were touched by this task ONLY via comment-string/event-name swaps — `ui-state-changed` → `notifications/ui_state/changed` — the flagged structure predates this diff):
- `inspectors-container.tsx` `m`-abbreviation, `InspectorsContainer` ~85 lines, `InspectorPanel` ~57 lines — pre-existing component structure; this diff changed only two doc-comment strings in that file.
- `views-context.tsx` `ViewsProvider` ~60 lines and missing doc comment — this diff changed only one doc-comment string in that file.
- Hardcoded test timeouts (`50`, `1500`) — pre-existing test scaffolding, not introduced here.

VERDICT ON THE TASK'S OWN CONTRACT (all PASS — these are the load-bearing review-focus items, none flagged by the engine):
1. Publish path correct: real mutation publishes `notifications/ui_state/changed` per-window with `{kind, state}`, deduped on `BindKey` (one publish per unique bridge). Confirmed in `commands.rs::publish_ui_state_changed_to` + `watcher.rs::recompute_and_emit_perspective_filters`.
2. Declared == raised: `#[notification] UiStateChanged { kind, state }` is the actual published struct (struct=payload via `from_declared`); coverage-guard `ui_state_changed_emitted_method_is_declared` asserts the helper's emitted method is in the declared `_meta` set — genuine drift catch.
3. Frontend swap complete: `listen("ui-state-changed")` → `subscribeUiStateChanged`; direct emits removed from `commands.rs` and `watcher.rs` (only comments mention the old name); `no-tauri-change-listeners.node.test.ts` adds `"ui-state-changed"` to the forbidden list as a reintroduction guard.
4. Real-pipeline test is genuine: `mutation_publishes_ui_state_changed_on_the_bridge` runs a real `UiState` mutation → `change.kind()` + `to_json()` → declared notification → real `NotificationBridge` → live subscriber asserts `{kind, state}` + provenance. Not a fixture/mock-boundary test.
5. Conventions honored: snapshot already in hand at publish time (no enrichment re-fetch — consistent with thin-events principle as the implementer documented); no `eprintln!` in touched Rust; "plane/bridge" jargon appears only in internal Rust source comments, not user/plugin-facing docs or strings.

Pre-existing failures disregarded per reviewer instructions (confirmed at HEAD 7a34f8496): `kanban-app::ai_panel_e2e` (GPU/model timeouts), `swissarmyhammer-plugin` `file_notes_e2e`/`example_layering_e2e` (CWD isolation).