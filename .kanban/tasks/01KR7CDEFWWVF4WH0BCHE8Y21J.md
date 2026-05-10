---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffc880
title: Make focus / jump / nav target the topmost layer (modal-layer model)
---
## The model (user's words, consolidated)

> "on a layer -- i want to be able to click or jump to focus, then navigate. when i inspect, a new layer for all inspectors is above the window - i want that to be focus on initial load, and click and jump is now focus and jump - ON THE TOP LAYER -- lower layers will only respond to a click to just dismiss higher layers -- much like `ESCAPE` when not captured by an editor needs to run the dismiss command."

> "i want you to make sure focus and dismiss are proper commands in our command system, not just hard coded."

> "showing the jump system -- a command is already there -- it needs to be smart to target the topmost layer"

> "dismiss needs to target the topmost layer -- though the bottom-most layer cannot really be dismissed"

There is a layer stack. The **topmost** (deepest pushed) layer is the **active layer**. The window layer is the bottom-most layer and cannot be dismissed.

| Event | Behavior | Wire via |
|-------|----------|----------|
| Click inside active layer's content | Focus that scope | `nav.focus` (NEW command) |
| Click outside active layer's content (backdrop / lower-layer area) | Dismiss the active layer | `app.dismiss` (existing) |
| Press `s` (jump) | Pills enumerate the active layer's scopes | `nav.jump` (existing) — make top-layer-aware |
| Jump-to match | Focus the matched scope | `nav.focus` (NEW command) |
| Arrow keys (not in editor) | Navigate within the active layer | (existing `nav.up/down/left/right` — already layer-scoped via snapshot) |
| `Escape` (not in editor) | Dismiss the active layer | `app.dismiss` (existing) |
| New layer mounts | Auto-focus its first interior scope | `nav.focus` (NEW command) |
| `app.dismiss` while window is the only layer | No-op | Existing handler returns early |

## What to do (in order)

### 1. Make `nav.jump` and `app.dismiss` top-layer-aware

Both commands already exist — do not reinvent them. The fix is inside their existing pathways.

#### `nav.jump`

`kanban-app/ui/src/components/jump-to-overlay.tsx::useJumpTargets` currently picks the enumeration layer from `priorFocusedFq`. Change it to use the topmost layer:

```ts
// before
const layerFq = priorFocusedFq !== null ? spatial.layerFqOf(priorFocusedFq) : null;
const fallbackLayerFq = layerFq ?? fqRoot(asSegment("window"));

// after
const layerFq = spatial.topLayerFq() ?? fqRoot(asSegment("window"));
```

The "prior focused FQM" plumbing remains for the dismiss-on-no-match restore-prior-focus path; only the enumeration target changes.

This needs `SpatialFocusActions.topLayerFq()` — see step 3.

#### `app.dismiss`

Audit the `app.dismiss` handler chain (frontend command → backend dispatch). It needs to be smart about the topmost layer:
- If the topmost layer is the inspector → close the topmost inspector panel (or all panels — match existing UX).
- If the topmost layer is the palette → close the palette.
- If the topmost layer is the jump-to overlay → close the overlay (already wired via the sentinel scope).
- If the topmost layer is the window (bottom-most) → no-op.

The backend likely already routes `app.dismiss` based on `paletteOpen` / `inspector_stack` state. Read `kanban-app/src/commands.rs` for the existing `app.dismiss` (or `app_dismiss`) handler. Verify it follows the topmost-layer rule. If it doesn't, fix it.

Then audit the **frontend** for any "close active layer" code that bypasses `app.dismiss`:

- `kanban-app/ui/src/components/inspectors-container.tsx` — the backdrop's `onClick={closeAll}` calls `dispatchInspectorCloseAll` directly. Replace with dispatching `app.dismiss` (so the backend's topmost-layer logic decides).
- `kanban-app/ui/src/components/jump-to-overlay.tsx` — the backdrop's `onClick` calls `handleDismiss()` directly (which restores prior focus then `onClose()`). This is acceptable because the jump-to overlay is its own sentinel-scoped layer with its own dismiss hook; or convert to dispatching `app.dismiss` and let the sentinel's shadow handle it. Match whichever pattern the palette already uses for consistency.

Direct close-this-specific-thing dispatches (e.g. `ui.inspector.close` from the X button on a panel header) stay as-is — they have a specific target, not "the active layer".

### 2. Add `nav.focus` as a new command

Currently focus claims are made by calling `useFocusActions().setFocus(fq)` directly from many places. Promote this to a real command.

Define `nav.focus` in `kanban-app/ui/src/components/app-shell.tsx::buildDynamicGlobalCommands` (next to `nav.jump`). Args: `{ fq: FullyQualifiedMoniker }`. Execute: calls `setFocus(fq)` via the existing kernel-facing pathway (one and only place that does so).

Then route every existing direct `setFocus(fq)` call through `dispatchCommand("nav.focus", { fq })`:

- `kanban-app/ui/src/components/focus-scope.tsx` — click handler.
- `kanban-app/ui/src/components/jump-to-overlay.tsx::useKeyMatcher` — unique-code match.
- `kanban-app/ui/src/components/entity-inspector.tsx::useFirstFieldFocus` — auto-claim on inspector mount.
- Any other non-null `setFocus(fq)` callsite (search via `code_context` `op: "search symbol"` and `Grep`).

`useFocusActions().setFocus(fq)` itself remains as the kernel-facing primitive — but only `nav.focus`'s execute closure calls it. Components stop calling `setFocus(fq)` directly.

The point: every focus claim flows through one auditable choke point. Behavior is uniform regardless of trigger (click / key / jump-to match / auto-claim). Tests can dispatch the command directly. Future cross-cutting concerns (logging, undo, animations, scroll-on-focus) hang off the command's execute, not off N call sites.

### 3. Add `SpatialFocusActions.topLayerFq()`

`kanban-app/ui/src/lib/spatial-focus-context.tsx`. Add `topLayerFq(): FullyQualifiedMoniker | null` returning the topmost (most-recently-pushed) layer FQM. Maintain a stack-top ref keyed off `pushLayer` / `popLayer` calls. Returns `null` when no layer is mounted (shouldn't happen in practice — the window layer is always pushed at app boot).

### 4. Revert iteration 3 — render-time push in `<FocusLayer>`

`kanban-app/ui/src/components/focus-layer.tsx` currently runs `pushLayer` and `registerLayerRegistry` at render time. Restore the prior `useEffect`-based shape via `git diff` against pre-iter-3.

Delete `inspector.first-field-focus-race.browser.test.tsx` — it pinned the render-time pattern.

### 5. Revert iteration 1 — per-panel `<FocusLayer>`

`kanban-app/ui/src/components/inspectors-container.tsx` currently wraps each panel in its own nested `<FocusLayer>`. Replace with the simpler shape:

- ONE outer `<FocusLayer name="inspector" parentLayerFq={windowLayerFq}>` (mounts when any panel is open).
- Inside, render panel content directly (no nested layer per panel, no `<FocusScope>` wrap around panel bodies). Field rows' `<FocusScope>`s register directly under the inspector layer.

Update or delete tests that pinned the per-panel-layer shape: `inspectors-container.layer-containment.browser.test.tsx`, `inspector.cross-panel-nav.browser.test.tsx`, `inspector.layer-shape.browser.test.tsx`, `inspector.entity-zone-barrier.browser.test.tsx`, `inspector.kernel-focus-advance.browser.test.tsx`, `inspectors-container.guards.node.test.ts`.

### 6. Auto-focus on inspector layer mount via `nav.focus`

When the inspector layer mounts (panel stack 0 → 1), dispatch `nav.focus` with the first field FQM of the topmost panel. Implementation:

- Locate the auto-claim in `<EntityInspector>::useFirstFieldFocus` (existing) or move to `<InspectorsContainer>` — whichever lands cleaner.
- Defer the dispatch by one tick (`queueMicrotask` or `setTimeout(0)`) so the inspector layer's own `useEffect` (registerLayerRegistry) has fired first.
- The dispatch is `nav.focus`, NOT a direct `setFocus(fq)`.

### 7. Fix the Jump-To overlay z-index

`kanban-app/ui/src/components/jump-to-overlay.tsx`. The `<JumpPill>` (~line 491) and the chrome backdrop (~line 354) have no `z-index`. Add `z-[80]` (above all known panel z-indices) to both so jump-to paints above the inspector panel when inspector is the active layer.

### 8. Tests

- **NEW** `jump-to-overlay.window-layer.browser.test.tsx` — no inspector open, press `s`, enumerated FQMs are window-layer scopes; pills paint at non-zero rects.
- **NEW** `jump-to-overlay.over-inspector.browser.test.tsx` — inspector open, press `s`, enumerated FQMs are inspector-layer scopes; pills paint at z > 30.
- **NEW** `inspectors-container.auto-focus-on-mount.browser.test.tsx` — open inspector from card-focused state, assert focused FQM is under `/window/inspector/...` within one frame.
- **NEW** `nav-focus.command.browser.test.tsx` — pin that `nav.focus` command exists, that its execute claims focus, and that `<FocusScope>` click handler dispatches `nav.focus` (not `setFocus` directly). Source-level guards forbidding direct `setFocus(` non-null calls in components.
- **NEW** `app-dismiss.topmost-layer.browser.test.tsx` — `app.dismiss` with no inspector is a no-op; with inspector open it closes the inspector; with palette open it closes the palette; precedence is topmost-first.
- **DROP/REWRITE** the iter-1 / iter-3 era tests listed above.
- Full UI suite green. Rust workspace green. tsc clean. clippy clean.

Tests must drive the same path the user hits in production. The kernel-simulator pattern is permissive — if it accepts a snapshot the real Rust kernel rejects, the test is a liability. Either avoid the simulator or strictify it.

## Acceptance Criteria

- [ ] `nav.focus` command exists in the global command registry. Args: `{ fq }`. Execute calls the existing kernel-facing `setFocus`. It is the ONE place in the UI that calls `setFocus(fq)` for non-null `fq`.
- [ ] Every callsite that previously called `setFocus(fq)` directly now dispatches `nav.focus`.
- [ ] `nav.jump` (existing) targets the topmost layer via `useJumpTargets` reading `topLayerFq()`.
- [ ] `app.dismiss` (existing) is smart about the topmost layer. With only the window layer mounted, it is a no-op. With inspector / palette / jump-to on top, it closes that layer.
- [ ] Backdrop and "close active layer" code in the frontend dispatches `app.dismiss` instead of hard-coding which layer to close.
- [ ] ONE shared `<FocusLayer name="inspector">` in `inspectors-container.tsx`. No per-panel layers. No `<FocusScope>` wrap per panel.
- [ ] `<FocusLayer>` primitive runs push/pop in `useEffect`. NO render-time side effects.
- [ ] `SpatialFocusActions.topLayerFq()` exists.
- [ ] No inspector open: pressing `s` paints pills on cards / columns / navbar. Click cards focuses them. Arrow keys navigate the board. Window layer keeps working as before.
- [ ] Inspector open: focus auto-claims to first field on mount via `nav.focus`. Pressing `s` paints pills on inspector field scopes. Pills render above inspector panel (z > 30). Arrow keys navigate fields. Click on inspector backdrop dispatches `app.dismiss` and dismisses the inspector.
- [ ] Full UI suite, Rust workspace, tsc, clippy all green/clean.

## Workflow

- Read iter-1 and iter-3 commits before touching anything.
- Order: command-system promotion FIRST (`nav.focus` + `topLayerFq()` + make `nav.jump` and `app.dismiss` top-layer-aware), THEN revert iter 1 and iter 3, THEN auto-focus on mount, THEN z-index. Each step verifiable by tests.
- Do not touch the `<FocusLayer>` primitive globally beyond reverting iter 3.
- Do not invent new commands. `nav.focus` is the only new one. `nav.jump` and `app.dismiss` exist — make them smarter, don't replace.

## History

Earlier review-finding sections (`2026-05-09`, `2026-05-10 — Production bug persists`, `2026-05-10 08:59 — Iteration-3 render-time fix`) describe failed iterations. Kept as record. Do NOT continue down their fix path — the model in this revised description supersedes them.

## 2026-05-10 — Iter-4 partial implementation, blocked on test-harness scope

### Done

- Iter-1 reverted: `inspectors-container.tsx`, `entity-inspector.tsx`, and the iter-1-era test files (`inspector.cross-panel-nav.browser.test.tsx`, `inspector.entity-zone-barrier.browser.test.tsx`, `inspector.kernel-focus-advance.browser.test.tsx`, `inspector.layer-shape.browser.test.tsx`, `inspectors-container.guards.node.test.ts`) restored to HEAD baseline.
- Iter-3 reverted: `focus-layer.tsx` restored to pre-render-time `useEffect`-based push/pop. Iter-3 test `inspector.first-field-focus-race.browser.test.tsx` deleted.
- Iter-1 test `inspectors-container.layer-containment.browser.test.tsx` deleted.
- `SpatialFocusActions.topLayerFq()` implemented with stack-top tracking via push/pop side effects in `spatial-focus-context.tsx`. Mock in `scroll-on-edge.test.ts` updated.
- `useJumpTargets` switched from `layerFqOf(priorFocusedFq)` to `topLayerFq()` for enumeration target.
- Jump-To overlay backdrop and `JumpPill` z-index set to `z-[80]`.
- `nav.focus` command added to `app-shell.tsx::buildDynamicGlobalCommands` with `{ args: { fq } }` shape; execute calls `setFocus(fq)` from the entity-focus actions ref. `CommandDef.execute` extended to receive `DispatchOptions` so per-call args reach the closure.
- `<FocusScope>` click and right-click handlers now dispatch `nav.focus({ args: { fq } })` instead of calling `spatial.focus(fq)` / `setFocus(fq)` directly.
- `<JumpToOverlay>` updated to use a `nav.focus` dispatcher (`navFocus` callback) for prior-focus restore, sentinel auto-claim, and unique-match landing — replacing all three `entity.setFocus(fq)` calls.
- `useFirstFieldFocus` updated to dispatch `nav.focus` with `queueMicrotask` deferral so the inspector layer's `useEffect` push runs first; restore-on-unmount continues to call `setFocus(prevOrNull)` directly because `nav.focus` is non-null only.
- Inspector backdrop click now dispatches `app.dismiss` instead of hard-coded `dispatchInspectorCloseAll`.
- Per-panel `<FocusScope moniker={entityZoneSegment}>` wrap removed from `<InspectorPanel>` per spec; doc-comments updated to describe the flat single-layer shape.
- `tsc --noEmit` clean after each phase.

### Blocker

After the above changes, **69 of 2081 UI tests fail** — all with the same shape: a test mounts `<FocusScope>` directly inside a `<CommandScopeProvider commands={[]}>` (no `<AppShell>`) and asserts that a click dispatches `spatial_focus` IPC. Click now dispatches `nav.focus`, which is registered only in `<AppShell>`. With no `nav.focus` in scope, dispatch falls through to backend `dispatch_command` IPC — which the test mocks don't recognize as triggering `spatial_focus`.

Failing test files include but are not limited to:
- `field.with-icon.browser.test.tsx` — three click-to-focus assertions
- `badge-list-nav.test.tsx` — pill click → leaf-key spatial_focus
- `perspective-tab-bar.no-inspect-on-dblclick.spatial.test.tsx` — single click → spatial_focus
- ~22 other test files following the same pattern

### Two ways forward (need user direction)

**Option A — Update test harnesses.** Modify ~25 test files to either:
(a) mount `<AppShell>` (heavy — pulls in keymap, palette, undo stack, busy provider);
(b) wrap each test's component in a `<CommandScopeProvider>` that registers a `nav.focus` definition matching production semantics.
Mechanical, large patch; preserves the spec's "nav.focus lives in app-shell" wording.

**Option B — Move `nav.focus` registration into `<EntityFocusProvider>`.** Wrap `EntityFocusProvider`'s children in a `<CommandScopeProvider>` that registers `nav.focus`. This makes `nav.focus` available wherever `setFocus` is — including all 25+ tests that mount `<EntityFocusProvider>`. Architecturally cleaner ("the command lives where the primitive it wraps is provided"), but deviates from the spec's "Define `nav.focus` in app-shell.tsx::buildDynamicGlobalCommands" wording.

The implement skill says do not deviate from the plan without user permission. **Stopping here for guidance.**

### Files changed (all under `kanban-app/ui/src/`)

- `lib/spatial-focus-context.tsx` — added `topLayerFq()` accessor and stack-top tracking.
- `lib/scroll-on-edge.test.ts` — added `topLayerFq` to the mock actions bag.
- `lib/command-scope.tsx` — `CommandDef.execute` now receives optional `DispatchOptions`; `runFrontendExecute` threads opts.
- `components/app-shell.tsx` — added `buildNavFocusCommand` and registered it in `buildDynamicGlobalCommands`.
- `components/jump-to-overlay.tsx` — top-layer-aware enumeration, z-index fix, `nav.focus` dispatcher replacing all three `entity.setFocus` calls.
- `components/focus-scope.tsx` — click + right-click now dispatch `nav.focus`.
- `components/entity-inspector.tsx` — `useFirstFieldFocus` now dispatches `nav.focus` deferred via `queueMicrotask`.
- `components/inspectors-container.tsx` — backdrop dispatches `app.dismiss`; per-panel `<FocusScope>` removed; doc-comments updated.

### Not yet done

- Updating other non-null `setFocus(fq)` callsites (board-view, data-table, grid-view, column-view, perspective-tab-bar, fields/field, cursor-focus-bridge, entity-focus-context::useFocusInto). These were left because Option A vs B will determine how much test-harness churn each new dispatch creates.
- Backend `app.dismiss` audit (already correct: palette → inspector → no-op).
- The 5 NEW browser tests listed in the spec.
- Verifying: `cargo nextest run --workspace`, `cargo clippy --all-targets --workspace -- -D warnings`.

## Review Findings (2026-05-10 17:18)

### Nits
- [x] `kanban-app/ui/src/components/nav-focus.source-guard.node.test.ts` — the `arg.includes(":")` heuristic on the scanned-line filter accepts any `setFocus(<arg>)` call whose arg contains a colon, including string-literal FQ calls like `setFocus("entity:foo")`. The intent is to skip TypeScript signature lines like `setFocus: (fq: ...) => void`, but a future regression that re-introduced a literal-FQ direct call would slip past. Tightening the heuristic — for example, requiring the line to also contain `: (` or `=>` to qualify as a signature, OR explicitly disallowing arg patterns that start and end with a quote — would close this blind spot. Not load-bearing today; future-proofing only.
- [x] `kanban-app/ui/src/components/nav-focus.source-guard.node.test.ts:43-48` — the allowlist entries for `kanban-app/ui/src/components/app-shell.tsx` and `kanban-app/ui/src/components/entity-inspector.tsx` are defensive: those files do not currently have any line that matches the regex `(\w+\.)?setFocus\(`. Both call sites use `setFocusRef.current(...)` (a non-matching shape) for the legitimate non-null primitive calls. The allowlist comments describe these as live exceptions, but they are actually future-proofing slots. Either tighten the comments to say "future-proofing for a direct-call shape — not currently matched" or scrub the allowlist entries until a real direct call appears. Doc/intent clarity nit only.
- [x] `kanban-app/ui/src/components/inspectors-container.tsx:72` — doc comment refers to `parentLayerKey` (the legacy variable name) while the actual prop on `<FocusLayer>` is `parentLayerFq`. Rename the reference in the comment to keep the prose consistent with the code.
