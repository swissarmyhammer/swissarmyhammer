---
assignees:
- claude-code
depends_on:
- 01KQYWPYZZ7T7VV8M8SAR5N2Z5
- 01KQYWR82C4HVZ1VTJPSBKDNST
position_column: todo
position_ordinal: e280
project: keyboard-navigation
title: JumpToOverlay component (portal, code labels, buffered key matcher)
---
## What

Create the visual + interactive overlay that the user sees when Jump-To is invoked. Architecture mirrors the inspector exactly (`kanban-app/ui/src/components/inspectors-container.tsx` + `entity-inspector.tsx` + `useFirstFieldFocus`):

- The overlay mounts its own `<FocusLayer name="jump-to">`.
- Inside that layer it mounts a single sentinel `<FocusScope>` and **claims focus** on it on open. This is critical: per `inspectors-container.tsx:96-105`, `nav.drillOut` walks the *currently-focused* scope's chain — without claiming focus into the jump-to layer first, Escape would walk `card → column → board → null` (the user's prior focus chain) before reaching dismiss. The inspector solves this with `useFirstFieldFocus`; we use the same pattern.
- The sentinel scope shadows `app.dismiss` via the `commands` prop on `<FocusScope>` (see `focus-scope.tsx:143`). The shadow's `execute` calls `handleDismiss()` (which restores prior focus, then calls the prop `onClose`).
- Escape flow: keymap → `nav.drillOut` → kernel returns same FQM (sentinel has no descent inside the jump-to layer) → `refs.dismissRef.current()` → `app.dismiss` resolves through the focused scope chain → sentinel's shadow runs → `handleDismiss`. No `stopPropagation`, no special-casing.

New file: `kanban-app/ui/src/components/jump-to-overlay.tsx`.

### Sentinel scope skeleton

```tsx
const handleDismiss = useCallback(() => {
  if (priorFocusedFqRef.current) {
    actions.setFocus(priorFocusedFqRef.current);
  }
  onClose();
}, [actions, onClose]);

const sentinelCommands = useMemo<CommandDef[]>(
  () => [{ id: "app.dismiss", name: "Dismiss Jump-To", execute: handleDismiss }],
  [handleDismiss],
);

return (
  <FocusLayer name={asSegment("jump-to")}>
    {createPortal(
      <FocusScope
        moniker={asSegment("jump-to-sentinel")}
        commands={sentinelCommands}
      >
        <div className="fixed inset-0 bg-black/30" onClick={handleDismiss} />
        {pills}
      </FocusScope>,
      document.body,
    )}
  </FocusLayer>
);
```

### Behavior

1. **Props**: `{ open: boolean; onClose: () => void; }`. Open state is owned by `app-shell.tsx` (next task wires it).

2. **On `open` going `true`**:
   - Capture `priorFocusedFq = actions.focusedFq()` BEFORE claiming focus. Stash in a ref so close-without-match can restore it.
   - Derive `priorLayerFq = actions.layerFqOf(priorFocusedFq)` (added by the enumerate task). If `null` (no prior focus, e.g., app just started) — fall back to enumerating the window root layer.
   - Call `actions.enumerateScopesInLayer(priorLayerFq)`. If the result is empty → immediately call `onClose()` and render nothing (don't claim focus, don't restore — focus is unchanged).
   - Generate codes via `generateSneakCodes(scopes.length)` and pair scopes 1:1.
   - Mount the overlay tree per the skeleton above.
   - Claim focus on the sentinel: after `<FocusScope moniker="jump-to-sentinel">` mounts, dispatch `actions.setFocus(sentinelFq)` (or whichever wrapper exists — `command-palette.tsx` and `useFirstFieldFocus` show working call sites). The kernel now sees focus inside the jump-to layer; the sentinel's `app.dismiss` shadow will be reachable via the cascade.
   - **Lock body scroll**: `document.body.style.overflow = "hidden"` on mount, restore prior value on unmount. Backdrop has `pointer-events: auto` and absorbs `wheel` / `touchmove` so internal scroll containers also stop scrolling.

3. **Render** (inside the portal):
   - A semi-transparent backdrop (`fixed inset-0 bg-black/30`) at the same z-tier the command palette uses (read `OVERLAY_OFFSET_ABOVE_TIER` from `focus-layer-z-tier-context.tsx`).
   - For each `{scope, code}` pair, an absolutely-positioned label badge at `(scope.rect.left + 4, scope.rect.top + 4)` containing the code (uppercase rendering, lowercase matching). Style: high-contrast pill (e.g., `bg-yellow-300 text-black font-mono px-1 rounded shadow`).
   - `data-jump-code={code}` and `data-jump-fq={fq}` attributes on each pill for deterministic e2e selection.

4. **Buffered key matching** (overlay's keydown handler — Escape is NOT in this handler; it flows through nav.drillOut → app.dismiss → sentinel shadow → handleDismiss):
   - Maintain `buffer: string` state.
   - On printable letter (`e.key.length === 1 && /[a-zA-Z]/.test(e.key)`):
     - `next = buffer + e.key.toLowerCase()`.
     - If `next` matches exactly one code → call `spatial_focus(scope.fq)` (or matching action wrapper), then `onClose()`. Focus is now on the matched scope; do NOT call `handleDismiss` (we don't want to restore prior focus over the match).
     - Else if `next` is a prefix of at least one code → `setBuffer(next)`.
     - Else (no match) → flash overlay (150ms red tint), then call `handleDismiss()` (restores prior focus, then onClose).
   - On `Backspace` → `setBuffer(buffer.slice(0, -1))` — never closes.
   - On other keys → ignored.

5. **Window blur**: while open, attach a `blur` listener on `window`; on blur call `handleDismiss()`. Standard modal hygiene.

6. **Backdrop click**: backdrop `onClick={handleDismiss}` per the skeleton.

### Dismiss summary table

| Trigger | Path | Focus result |
|---|---|---|
| `Escape` | global keymap → `nav.drillOut` → no descent inside jump-to → `app.dismiss` → sentinel's `commands` shadow → `handleDismiss` | restore prior focus |
| Backdrop click | backdrop `onClick={handleDismiss}` | restore prior focus |
| Letter not extending any code prefix | overlay's keydown → 150ms flash → `handleDismiss` | restore prior focus |
| Unique multi-letter match | overlay's keydown → `spatial_focus(fq)` → `onClose` (skip restore) | focus on matched scope |
| `Backspace` | overlay's keydown | shrink buffer; never closes |
| Window blur | window `blur` listener → `handleDismiss` | restore prior focus |
| 0 visible scopes when invoked | open-effect immediate `onClose` | unchanged (overlay never claimed focus) |

## Acceptance Criteria

- [ ] Component renders nothing when `open === false`.
- [ ] When opened with 0 enumerable scopes, immediately calls `onClose()` without claiming focus or restoring.
- [ ] Mounted via `<FocusLayer name="jump-to">` containing one `<FocusScope>` sentinel whose `commands` prop registers an `app.dismiss` shadow calling `handleDismiss`.
- [ ] On open, claims focus on the sentinel scope so `nav.drillOut` cascades correctly.
- [ ] Each pill has `data-jump-code` and `data-jump-fq` attributes.
- [ ] Body scroll is locked while open (`document.body.style.overflow === "hidden"`); restored on close.
- [ ] Wheel events on the backdrop do not scroll any underlying scroll container.
- [ ] Typing a unique code calls `spatial_focus(fq)` exactly once then `onClose()`; focus ends on the matched scope (no restore).
- [ ] Typing a prefix of multiple codes narrows without dispatching focus or closing.
- [ ] Typing a non-matching letter flashes red briefly, restores prior focus, then closes.
- [ ] `Escape` closes via `nav.drillOut → app.dismiss → sentinel shadow → handleDismiss`; the overlay's keydown handler does NOT see Escape; prior focus is restored.
- [ ] `Backspace` shrinks the buffer; an empty-buffer Backspace does NOT close.
- [ ] Backdrop click closes; prior focus is restored.
- [ ] Window blur closes; prior focus is restored.
- [ ] While open, global keybindings other than `nav.drillOut` / `app.dismiss` do not fire.

## Tests

- [ ] New browser-mode test `kanban-app/ui/src/components/jump-to-overlay.browser.test.tsx`:
  - Mount under the real provider tree (SpatialFocusProvider + the App's CommandScopeProvider hierarchy) with several seeded `<FocusScope>`s in a parent layer and known mocked rects.
  - Pre-set focus to one of those scopes (the "prior" focus).
  - Render `<JumpToOverlay open onClose={onCloseSpy} />`.
  - Assert pills appear with `data-jump-code` / `data-jump-fq` at expected positions; assert focus is now on the sentinel (read `actions.focusedFq()`).
  - Assert `document.body.style.overflow === "hidden"`; assert restored on unmount.
  - Type a code's letters → assert `spatial_focus` invoked with matching FQM, `onCloseSpy` called once, focus ends on the matched scope (NOT restored to prior).
  - Fire `Escape` at the document level → assert `onCloseSpy` was called, no `spatial_focus` dispatched, the trace shows `nav.drillOut` → `app.dismiss` were dispatched (use the existing inspector-dismiss test helper or spy on the dispatcher), AND focus was restored to the prior FQM.
  - Type a non-matching letter → assert flash class is applied, then after the 150ms timer (use `vi.useFakeTimers()`) `onCloseSpy` called and prior focus restored.
  - Type `Backspace` with empty buffer → assert `onCloseSpy` NOT called.
  - Stub `enumerateScopesInLayer()` to return `[]` → render with `open` → assert immediate `onCloseSpy`, nothing rendered, focus NOT changed.
  - Fire `blur` on `window` → assert `onCloseSpy`, prior focus restored.
  - 30 scopes (forces 2-letter codes): first letter → no dispatch, overlay open; second letter → dispatch + close.
- [ ] Test command: `cd kanban-app/ui && pnpm test jump-to-overlay` — passes.

## Workflow

- Use `/tdd` — write the browser test first; then implement the component; re-run. Before writing the implementation, read `entity-inspector.tsx`'s `useFirstFieldFocus` and `inspectors-container.tsx`'s layer mount for the focus-claim pattern, and `focus-scope.tsx:143` for the `commands` prop signature. #nav-jump