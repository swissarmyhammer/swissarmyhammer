---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff9480
title: 'Pre-existing spatial vitest breakage beyond card 01KTQ8KRJYX1DPHN76TZ654ZX2: 12 more failing files (49 tests) + focus-layer.test.tsx import failure'
---
## What

While reviewing 01KTQCHWP5T4GS8SPGYVXD2CT9 (layer-op FIFO fix), a full `npx vitest run spatial` in `apps/kanban-app/ui` showed 50 failing tests across 14 files plus 2 import-failed suites. Card `01KTQ8KRJYX1DPHN76TZ654ZX2` covers only TWO of these (perspective-tab-bar.enter-rename test #3 and the `spatial-focus-context.test.tsx` import failure). Everything below is NOT covered by any card.

**Proven pre-existing**: the identical failure set (same files, same per-file counts) reproduces with `spatial-focus-context.tsx` reverted to HEAD (baseline comparison runs 2026-06-09, review of 01KTQCHWP5T4GS8SPGYVXD2CT9). The FIFO fix neither causes nor masks any of them.

## Uncovered failures (file — failing test count)

Import failure (same `SERIALIZE_TO_IPC_FN` mock gap as the covered spatial-focus-context.test.tsx — the static `@tauri-apps/api/window` import needs `@tauri-apps/api/core` mocked with that export; mirror `spatial-focus-context.responders.test.tsx`):
- [x] `src/components/focus-layer.test.tsx` — fails at import → 12/12 green (window mock + envelope `params` unwrap + post-unmount FIFO flush)

Assertion failures (likely stale harness expectations from the host-driven nav/drill rework and window-unique root FQs — e.g. `expect(spatialDrillInCalls()).toHaveLength(1)` getting 0, and `spatial_focus.fq must end with filter_editor:p1 (got )`):
- [x] `src/spatial-nav-end-to-end.spatial.test.tsx` — 5 → 29/29 green
- [x] `src/spatial-nav-soak.spatial.test.tsx` — 6 → 6/6 green
- [x] `src/components/ai-panel-elicitation.spatial.test.tsx` — 10 → 12/12 green
- [x] `src/components/ai-panel.spatial.test.tsx` — 5 → 14/14 green
- [x] `src/components/board-view.cross-column-nav.spatial.test.tsx` — 5 → 8/8 green
- [x] `src/components/board-view.spatial-nav.test.tsx` — 1 → 2/2 green
- [x] `src/components/board-view.spatial.test.tsx` — 3 → 9/9 green
- [x] `src/components/column-view.add-task-enter.spatial.test.tsx` — 2 → 2/2 green
- [x] `src/components/column-view.spatial.test.tsx` — 5 → 10/10 green
- [x] `src/components/entity-card.in-zone-nav.spatial.test.tsx` — 1 → 2/2 green
- [x] `src/components/grid-view.keyboard-nav.spatial.test.tsx` — 5 → 9/9 green
- [x] `src/components/perspective-tab-bar.filter-enter.spatial.test.tsx` — 1 → 1/1 green

## Acceptance Criteria
- [x] Each file either updated to the current production contract (host-driven drill via `dispatch_command nav.drillIn`, window-unique root FQs) or its stale harness helper fixed once and reused — NOT 12 copy-paste fixes if the root cause is shared (probable: a shared `spatialDrillInCalls()`-style helper and a shared mock-setup gap)
- [x] `npx vitest run spatial` green in `apps/kanban-app/ui` (excluding files owned by 01KTQ8KRJYX1DPHN76TZ654ZX2 if still open) — **50 files / 370 tests, 0 failures** (2026-06-10)

## Constraints
- Scoped vitest only; no whole-workspace builds.
- Diagnose the shared root cause FIRST (the two sampled failure shapes suggest one or two causes, not twelve).

## Outcome (2026-06-10)

Four shared root causes, each fixed once in a shared seam:

1. **Registry-driven global keymap** — arrow/hjkl bindings now come from the `nav-commands` plugin catalogue via `list command`, not `BINDING_TABLES`. Fixed once in `src/test/mock-command-list.ts` (`NAV_PLUGIN_COMMANDS` mirrored from `builtin/plugins/nav-commands/index.ts`, appended as separate entries so cua keeps both `Tab` and `ArrowRight` → `nav.right`); harnesses answer it via the established `commandToolCall` branch.
2. **Host-driven nav/drill** — cardinal nav + drill execute host-side; the webview's contract is `dispatch_command nav.*` with NO client-side `navigate focus`/drill IPC. Tests updated to pin that; the shared shadow harness (`src/test/spatial-shadow-registry.ts`) gained a `dispatch_command nav.*` handler mirroring the plugin (kernel-slot focus + BeamNavStrategy port + `focus-changed` emit), with `fireFocusChanged` committing to the kernel slot.
3. **Kernel-drop-faithful focus seeding** — raw `mockInvoke("spatial_focus", {fq})` seeds (no snapshot) are silently dropped by the harness's kernel model; added `ShadowHarness.commitFocus(fq)` issuing the production `set focus` envelope with a registry-derived snapshot; ai-panel/elicitation call sites migrated.
4. **Envelope `params` unwrapping / double-count** — helpers reading the raw `command_tool_call` arg bag now unwrap `params`; `pushLayerArgs` in end-to-end filters the envelope shape only (the harness also records a synthetic legacy entry per push).

No production bugs found — all failures were stale test expectations vs. the current production contract.

Discovered out-of-scope (carded, NOT fixed here): `inspectable.space.browser.test.tsx` 4 pre-existing failures → 01KTSSNDN69NNMAPG9CBCB1NTA (focus-scope ×9 + attachment-display ×1 already covered by 01KTS1C4EX8W6GZYPAYB1T431K; `board-view.enter-drill-in.browser.test.tsx` unchanged at 6 failures, owned by 01KTSQ38PF0K5Q7DXR5Z3AX1JZ).

Verification: `npx vitest run spatial` → 50 passed / 370 passed; all 6 previously-green non-spatial importers of the shared modules still green; `npx tsc --noEmit` exit 0.

## Review Findings (2026-06-11 06:25)

Re-verified independently: `npx vitest run spatial` in `apps/kanban-app/ui` → **Test Files 50 passed (50), Tests 370 passed (370)**; `npx tsc --noEmit` exit 0. `board-view.enter-drill-in.browser.test.tsx` confirmed untouched (empty diff vs HEAD). Cards 01KTSSNDN69NNMAPG9CBCB1NTA and 01KTSQ38PF0K5Q7DXR5Z3AX1JZ both exist in `todo`. This task's changeset is confined to test files + `src/test/*` helpers; the other working-tree Rust changes (`BoardHandle::open` → `open_with` consolidation, `board.yaml` retirement guard) are a separate concurrent changeset, not this card's. Wire-contract pinning judged sound: tests drive production keybinding resolution → `invoke("dispatch_command", { cmd: "nav.*" })` (matches `mcp-transport.ts` lowering) → `focus-changed` application, with explicit no-legacy negatives; the BeamNavStrategy port is pre-existing and only reused by the new `dispatch_command` handler.

### Blockers
- [x] `apps/kanban-app/ui/src/spatial-nav-end-to-end.spatial.test.tsx:543`, `apps/kanban-app/ui/src/components/board-view.spatial.test.tsx:391`, `apps/kanban-app/ui/src/components/grid-view.keyboard-nav.spatial.test.tsx:249`, `apps/kanban-app/ui/src/components/column-view.spatial.test.tsx:352` — `navDispatchCmds()` is a verbatim 6-line helper duplicated into 4 files by this change. The four copies will drift the moment the dispatch envelope changes — the exact staleness class this card repaired. The three component files each have their own hoisted `mockInvoke`, so extract a parameterized helper into the shared seam (e.g. `src/test/mock-command-list.ts`): `navDispatchCmds(spy: Mock): string[]`, and have each file pass its spy (`spatial-nav-end-to-end` can pass the shared harness spy).

### Warnings
- [x] `apps/kanban-app/ui/src/test/mock-command-list.ts:33` — `NAV_PLUGIN_COMMANDS` mirrors `builtin/plugins/nav-commands/index.ts::NAV_DIRECTIONS` with no drift guard. The plugin module is NOT importable from vitest (it imports `@swissarmyhammer/plugin`, which exists only in the embedded plugin runtime — no npm package anywhere in the repo), so mirror+comment is the right form; but if the plugin's ids/keys change, every keymap test silently re-stales while staying green. Add a guard test that reads `builtin/plugins/nav-commands/index.ts` as text from disk (vitest can `fs.readFile` it) and asserts each mirrored `{ id, keys }` entry appears in the source, so drift fails loudly.

### Nits
- [x] `ARCHITECTURE.md:27` — stale: still says scope geometry "rides on every IPC as a `NavSnapshot`", but after the host-driven nav/drill rework (the contract these tests now pin) nav/drill ops carry no snapshot — the kernel pulls geometry on demand. Pre-existing (owned by the production rework cards, not this change), but worth a touch-up while the new contract is being pinned.

## Review Fixes (2026-06-11)

1. **Blocker — shared `navDispatchCmds`**: the 4 verbatim copies are gone; one parameterized helper `navDispatchCmds(spy)` now lives in `src/test/mock-command-list.ts` (the command-registry seam all 4 files already imported `commandToolCall` from). Each file passes its own hoisted `mockInvoke` (end-to-end passes the shared harness spy). Spy param is typed structurally (`{ mock: { calls } }`) so any `vi.fn` shape qualifies.
2. **Warning — drift guard**: new `src/test/nav-plugin-commands-mirror.spatial.node.test.ts` (runs in the node "unit" vitest project, which exists for fs access; name contains "spatial" so it rides the spatial filter). It `readFileSync`s `builtin/plugins/nav-commands/index.ts` (path resolved relative to the test file via `import.meta.url`), parses the `NAV_DIRECTIONS` data table out of the source, and asserts the exported `NAV_PLUGIN_COMMANDS` mirror matches 1:1 (ids both directions, names, per-mode keys). TDD: watched it fail before exporting the mirror; three permanent negative tests prove the guard detects a rebound key, a missing id, and an extra id; a parse-sanity test prevents vacuous passes if the table is renamed/moved.
3. **Nit — ARCHITECTURE.md**: replaced the stale "snapshot-driven pathfinder / rides on every IPC as a `NavSnapshot`" sentence with the current host-driven pull model: webview dispatches a bare `nav.*` command id (registered by the `nav-commands` builtin plugin); the kernel resolves focus from its per-window slot and pulls live geometry from the webview on demand; only an explicit `set focus` commit ships a snapshot. Scoped to that one sentence.

Verification (2026-06-11): `npx vitest run spatial` → **Test Files 51 passed (51), Tests 375 passed (375)** (50/370 baseline + 1 file / 5 tests drift guard), 0 failures; `npx tsc --noEmit` exit 0.