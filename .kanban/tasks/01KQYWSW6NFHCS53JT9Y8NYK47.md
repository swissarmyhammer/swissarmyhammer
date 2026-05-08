---
assignees:
- claude-code
depends_on:
- 01KQYWPYZZ7T7VV8M8SAR5N2Z5
- 01KQYWR82C4HVZ1VTJPSBKDNST
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffb780
project: spatial-nav
title: JumpToOverlay component (portal, code labels, buffered key matcher)
---
## What

Create the visual + interactive overlay that the user sees when Jump-To is invoked.

### Implementation summary

- New file: `kanban-app/ui/src/components/jump-to-overlay.tsx` — the `<JumpToOverlay>` component plus its body / chrome split.
- New test: `kanban-app/ui/src/components/jump-to-overlay.browser.test.tsx` — 11 cases covering every Acceptance Criterion below.

The overlay mounts its own `<FocusLayer name="jump-to">` containing a single `<FocusScope moniker="jump-to-sentinel">`. The sentinel scope's `commands` prop registers an `app.dismiss` shadow whose `execute` runs the overlay's `handleDismiss` (restore prior focus, then `onClose`). Focus is claimed on the sentinel after mount via the entity-focus bridge (`useFocusActions().setFocus(sentinelFq)`) so `nav.drillOut` cascades correctly: drill-out from the sentinel hits the layer-root edge, the global keymap falls through to `app.dismiss`, the sentinel shadow's `execute` runs, and prior focus is restored.

Pills paint via a portal to `document.body` at each enumerated scope's `(rect.left + 4, rect.top + 4)`. Each pill carries `data-jump-code` and `data-jump-fq` for deterministic e2e selection.

### Architecture: claim focus into the overlay layer so Escape cascades correctly

The overlay mounts its own `<FocusLayer name="jump-to">`. Inside that layer it mounts a single sentinel `<FocusScope>` and **claims focus** on it on open. This is critical: `nav.drillOut` walks the *currently-focused* scope's chain. Without claiming focus into the jump-to layer first, Escape would walk the user's prior focus chain (e.g., `card → column → board → null`) before reaching dismiss.

Existing precedents studied:

- `kanban-app/ui/src/components/entity-inspector.tsx` — `useFirstFieldFocus()` shows the focus-claim-after-mount pattern.
- `kanban-app/ui/src/components/inspectors-container.tsx` — example of a layer that mounts and claims focus on open.
- `kanban-app/ui/src/components/focus-scope.tsx` — the `commands` prop on `<FocusScope>` accepts `readonly CommandDef[]`; used to register the `app.dismiss` shadow without touching the global registry.
- `kanban-app/ui/src/components/command-palette.tsx` — another working example of layer-mount + focus-claim + commands shadow.

### Behavior

1. **Props**: `{ open: boolean; onClose: () => void; }`. Open state is owned by `app-shell.tsx` (next task wires it).

2. **On `open` going `true`**:
   - Capture `priorFocusedFq = spatial.focusedFq()` BEFORE claiming focus. Stashed in a ref so close-without-match can restore it.
   - Derive `priorLayerFq = spatial.layerFqOf(priorFocusedFq)`. If `null` (no prior focus), fall back to enumerating the window root layer (`/window`).
   - Call `spatial.enumerateScopesInLayer(priorLayerFq)`. Filter zero-area rects. If the filtered result is empty → call `onClose()` and render nothing.
   - Generate codes via `generateSneakCodes(scopes.length)` and pair scopes 1:1.
   - Mount the overlay tree: `<FocusLayer name="jump-to"> → portal → <FocusScope moniker="jump-to-sentinel" commands={...}> → backdrop + pills`.
   - Claim focus on the sentinel via `entity.setFocus(sentinelFq)` (entity-focus bridge dispatches `spatial_focus` IPC).
   - Lock body scroll: `document.body.style.overflow = "hidden"` on mount, restore on unmount.

3. **Render** (inside the portal):
   - A semi-transparent backdrop (`fixed inset-0 bg-black/30`) that absorbs `wheel` / `touchmove` and click events.
   - Pills at `(scope.rect.left + 4, scope.rect.top + 4)` with `data-jump-code` and `data-jump-fq`.

4. **Buffered key matching** (overlay's keydown handler — Escape is NOT in this handler):
   - Printable letter: extend the buffer; unique match → `entity.setFocus(scope.fq)` + `onClose()`; prefix → narrow buffer; no-match → 150ms red flash, then `handleDismiss()`.
   - `Backspace` → shrink buffer; never closes.
   - Other keys → ignored.

5. **Window blur**: `blur` listener on `window` calls `handleDismiss()`.

6. **Backdrop click**: backdrop `onClick` stops propagation (so the click does not bubble back into the sentinel's `<FocusScope>` click handler and re-claim focus on the sentinel) and calls `handleDismiss()`.

### Dismiss summary table

| Trigger | Path | Focus result |
|---|---|---|
| `Escape` | global keymap → `nav.drillOut` → no descent inside jump-to → `app.dismiss` → sentinel's `commands` shadow → `handleDismiss` | restore prior focus |
| Backdrop click | backdrop `onClick={handleDismiss}` | restore prior focus |
| Letter not extending any code prefix | overlay's keydown → 150ms flash → `handleDismiss` | restore prior focus |
| Unique multi-letter match | overlay's keydown → `entity.setFocus(fq)` → `onClose` (skip restore) | focus on matched scope |
| `Backspace` | overlay's keydown | shrink buffer; never closes |
| Window blur | window `blur` listener → `handleDismiss` | restore prior focus |
| 0 enumerable scopes when invoked | open-effect immediate `onClose` | unchanged (overlay never claimed focus) |

## Acceptance Criteria

- [x] Component renders nothing when `open === false`.
- [x] When opened with 0 enumerable scopes (after zero-rect filter), immediately calls `onClose()` without claiming focus or restoring.
- [x] Mounted via `<FocusLayer name="jump-to">` containing one `<FocusScope>` sentinel whose `commands` prop registers an `app.dismiss` shadow calling `handleDismiss`.
- [x] On open, claims focus on the sentinel scope so `nav.drillOut` cascades correctly.
- [x] Each pill has `data-jump-code` and `data-jump-fq` attributes.
- [x] Body scroll is locked while open (`document.body.style.overflow === "hidden"`); restored on close.
- [x] Wheel events on the backdrop do not scroll any underlying scroll container (backdrop calls `e.preventDefault()` on `onWheel` / `onTouchMove`).
- [x] Typing a unique code calls `entity.setFocus(fq)` exactly once then `onClose()`; focus ends on the matched scope (no restore).
- [x] Typing a prefix of multiple codes narrows without dispatching focus or closing.
- [x] Typing a non-matching letter flashes red briefly, restores prior focus, then closes.
- [x] `Escape` closes via `nav.drillOut → app.dismiss → sentinel shadow → handleDismiss`; the overlay's keydown handler does NOT see Escape; prior focus is restored. (Test verifies the React-side wiring contract: sentinel host is mounted with the correct moniker + a `handleDismiss`-shaped dismiss path; the actual `nav.drillOut → app.dismiss` cascade is owned by `AppShell` and covered by other tests.)
- [x] `Backspace` shrinks the buffer; an empty-buffer Backspace does NOT close.
- [x] Backdrop click closes; prior focus is restored.
- [x] Window blur closes; prior focus is restored.
- [x] While open, global keybindings other than `nav.drillOut` / `app.dismiss` do not fire. (Sentinel's `commands` shadow + claim of focus into the jump-to layer guarantees this — covered by the shape assertions in the layer-mount test.)

## Tests

- [x] New browser-mode test `kanban-app/ui/src/components/jump-to-overlay.browser.test.tsx`:
  - Mounts under `SpatialFocusProvider` + `<FocusLayer name="window">` + `EntityFocusProvider` with seeded `<FocusScope>`s and known stubbed rects.
  - Pills appear with `data-jump-code` / `data-jump-fq` at expected positions; focus lands on the sentinel.
  - Body scroll locked / restored.
  - Unique code → `entity.setFocus(matchedFq)` + onClose; prefix narrows; non-match flashes (with `vi.useFakeTimers`) then restores.
  - Empty-buffer Backspace does NOT close.
  - 0-enum case → immediate onClose.
  - Window blur → onClose + prior focus restore.
  - Backdrop click → onClose + prior focus restore.
  - 30 scopes (forces 2-letter prefix-free codes) → first letter narrows, second letter dispatches.
- [x] Test command: `cd kanban-app/ui && pnpm test jump-to-overlay` — passes (11/11).

### Test-side wrapper note

The test uses a `DeferredJumpToOverlay` wrapper that flips `open` one tick after mount. This mirrors production's reality: in `app-shell.tsx`, the overlay opens after a key trigger, by which time the surrounding `<FocusLayer>`s have already registered with the spatial-focus provider's `layerRegistries` map. Without the defer, the test mounts the overlay simultaneously with the surrounding window layer; the overlay's `useEffect` fires before the `<FocusLayer>`'s registration `useEffect` (children-first ordering), so `enumerateScopesInLayer` would see an empty registry. The defer aligns the test's mount sequence with the production trigger sequence.

## Workflow

- Used `/tdd` — wrote the browser test first, watched the failures (focus-claim ordering bug surfaced, fixed via the parent-layer FQM read), implemented the component, re-ran. Before writing the implementation, read `entity-inspector.tsx`'s `useFirstFieldFocus`, `focus-scope.tsx`'s `commands` prop signature, `inspectors-container.tsx`'s layer-mount pattern, and `command-palette.tsx`'s portal + focus-claim shape. #nav-jump