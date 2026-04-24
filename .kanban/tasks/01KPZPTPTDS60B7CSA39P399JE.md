---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
position_column: todo
position_ordinal: ff8980
project: spatial-nav
title: Dialog/confirm overlays wrap in their own FocusLayer
---
## What

Every modal overlay — confirmation dialogs, alert dialogs, the command palette, date pickers rendered as popovers, etc. — wraps its content in `<FocusLayer name="...">`. This generalizes the inspector-per-panel model from card `01KNQXYC4RBQP1N2NQ33P8DPB9` to everything else that visually sits "on top of" the rest of the UI.

### Why

- A confirm dialog opened from within an inspector field should **capture** keyboard nav — arrow keys move between Confirm/Cancel buttons, not back to the inspector fields beneath. That's a layer boundary.
- When the dialog closes, focus must return to the field that opened it, not to the inspector's `last_focused` at some earlier time. That's layer pop + `last_focused` on the dialog's parent layer.
- A dialog opened directly from the board (no inspector in the picture) has the window root as its parent, not an inspector.
- In a multi-window world, a dialog in window A is entirely isolated from anything in window B — which the layer forest gives us for free because the dialog's `window_label` is derived from the Tauri webview at `spatial_push_layer` time.

### Components to wrap

Inventory the existing modal/overlay components in `kanban-app/ui/src/components/` and wrap each:

- **Command palette** (`app-shell.tsx` or wherever `<CommandPalette>` is rendered) → `<FocusLayer name="palette">`
- **Quick capture** (if it's rendered as an overlay in the main window rather than its own Tauri window)
- **Confirm dialog** (if one exists) → `<FocusLayer name="dialog">`
- **Alert dialog** (shadcn `<AlertDialog>`) → `<FocusLayer name="dialog">`
- **Context menus** — decide: are context menus their own layer, or transient UI that doesn't get keyboard focus? Default: **not a layer**; they use native menu focus, disappear on outside click. Document this decision.
- **Date pickers / combo-box popovers** — decide: these are part of the field they belong to and don't need their own layer, since they contain only one interactive element at a time. Document this decision.

Run a sweep: grep for uses of shadcn `<Dialog>`, `<AlertDialog>`, `<Popover>`, `<Sheet>` and decide layer vs. not-a-layer for each, based on whether multi-element keyboard nav is meaningful inside it.

### FocusLayer parent wiring

A dialog opened from inside an inspector panel should have that inspector panel's layer as its parent — not the window root. Since most dialogs are rendered via portals (to `document.body`), the React ancestor context for FocusLayer may point to the window root, not the inspector. Two options:

- (a) Render the dialog inline (no portal) so the React ancestor chain reflects the logical parent. Breaks for shadcn's Radix-based primitives that always portal.
- (b) Have the dialog explicitly receive its parent layer key. Anywhere a dialog is opened, the opener passes its own layer key (read from `useFocusLayer()` hook). The dialog component uses `<FocusLayer parentLayerKey={parentKey}>`.

**Choose (b)** — same pattern as the inspector-stack wiring in card `01KNQXYC4RBQP1N2NQ33P8DPB9`. Add a `useCurrentLayerKey()` hook that returns the nearest ancestor layer key from context (at the opener site, where portals don't interfere).

### Files to create/modify

- `kanban-app/ui/src/components/focus-layer.tsx` — no changes beyond what card `01KNQXYC4RBQP1N2NQ33P8DPB9` already adds (optional `parentLayerKey` prop)
- `kanban-app/ui/src/lib/entity-focus-context.tsx` — add `useCurrentLayerKey()` hook
- Each dialog/overlay component — wrap internal content in `<FocusLayer name="..." parentLayerKey={...}>`
- Each opener site — read its own layer key and pass to the dialog

### Subtasks
- [ ] Add `useCurrentLayerKey()` hook reading from `FocusLayerContext`
- [ ] Audit modal/overlay components; decide layer-or-not for each; document
- [ ] Wrap confirm/alert dialogs in `<FocusLayer name="dialog" parentLayerKey={...}>`
- [ ] Wrap command palette in `<FocusLayer name="palette" parentLayerKey={windowLayerKey}>`
- [ ] Add tests for dialog-on-inspector and dialog-on-window scenarios

## Acceptance Criteria
- [ ] Every modal overlay that supports multi-element keyboard nav wraps its content in a FocusLayer
- [ ] A dialog's parent layer reflects where it was opened from (inspector panel, window root, etc.), not the React portal parent
- [ ] Arrow keys inside a dialog move between the dialog's own controls only — nothing beneath leaks in
- [ ] Closing the dialog pops its layer and restores focus to the opener's last_focused (on the parent layer)
- [ ] Command palette is its own layer under the window root; closing it returns focus to whatever was focused in the window before
- [ ] Dialogs in different windows are isolated via window_label on their layer
- [ ] Context menus, popovers, and date pickers (single-control overlays) remain as non-layer transient UI; this decision is documented in the FocusLayer component file
- [ ] `cargo test` and `pnpm vitest run` pass

## Tests

- [ ] `focus-layer.test.tsx` — `parentLayerKey` prop overrides ancestor context when present
- [ ] `confirm-dialog.test.tsx` (or equivalent) — opens from inside inspector panel, gets that panel's layer as parent; closing restores focus to the opener field
- [ ] Palette test — palette's layer parent is window root; closing palette restores window root's last_focused
- [ ] Rust: `ancestors_of(dialog_key)` includes inspector, then window root, in that order
- [ ] Rust: nav inside dialog sees only dialog's entries — sibling window entries invisible
- [ ] Run `cargo test` and `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.