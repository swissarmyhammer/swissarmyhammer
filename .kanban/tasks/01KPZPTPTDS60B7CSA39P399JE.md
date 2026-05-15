---
assignees:
- claude-code
depends_on:
- 01KNQXYC4RBQP1N2NQ33P8DPB9
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffb080
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
- [x] Add `useCurrentLayerKey()` hook reading from `FocusLayerContext`
- [x] Audit modal/overlay components; decide layer-or-not for each; document
- [x] Wrap confirm/alert dialogs in `<FocusLayer name="dialog" parentLayerKey={...}>`
- [x] Wrap command palette in `<FocusLayer name="palette" parentLayerKey={windowLayerKey}>`
- [x] Add tests for dialog-on-inspector and dialog-on-window scenarios

## Acceptance Criteria
- [x] Every modal overlay that supports multi-element keyboard nav wraps its content in a FocusLayer
- [x] A dialog's parent layer reflects where it was opened from (inspector panel, window root, etc.), not the React portal parent
- [x] Arrow keys inside a dialog move between the dialog's own controls only — nothing beneath leaks in
- [x] Closing the dialog pops its layer and restores focus to the opener's last_focused (on the parent layer)
- [x] Command palette is its own layer under the window root; closing it returns focus to whatever was focused in the window before
- [x] Dialogs in different windows are isolated via window_label on their layer
- [x] Context menus, popovers, and date pickers (single-control overlays) remain as non-layer transient UI; this decision is documented in the FocusLayer component file
- [x] `cargo test` and `pnpm vitest run` pass

## Tests

- [x] `focus-layer.test.tsx` — `parentLayerKey` prop overrides ancestor context when present
- [x] `confirm-dialog.test.tsx` (or equivalent) — opens from inside inspector panel, gets that panel's layer as parent; closing restores focus to the opener field
- [x] Palette test — palette's layer parent is window root; closing palette restores window root's last_focused
- [x] Rust: `ancestors_of(dialog_key)` includes inspector, then window root, in that order
- [x] Rust: nav inside dialog sees only dialog's entries — sibling window entries invisible
- [x] Run `cargo test` and `cd kanban-app/ui && npx vitest run` — all pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

## Implementation Notes (2026-04-26)

### Audit results

Swept `kanban-app/ui/src/components/` for shadcn `<Dialog>`, `<AlertDialog>`, `<Popover>`, `<Sheet>`, plus modal-shaped fixed-overlay markers (`createPortal`, `fixed inset-0`, `aria-modal`). The findings:

| Surface | Decision | Rationale |
| --- | --- | --- |
| Command palette (`command-palette.tsx`) | **Layer** (`name="palette"`) | Multi-element keyboard nav between input and result rows; arrow keys must be captured from anything beneath. |
| Inspector panel stack (`inspectors-container.tsx`) | **Layer** (`name="inspector"`) — covered by parent task `01KNQXYC4RBQP1N2NQ33P8DPB9`. | Multi-element keyboard nav between fields and across panels. |
| Quick capture (`quick-capture.tsx`) | **Layer** (`name="window"`) — its own Tauri webview, not an in-window overlay. | Each Tauri webview's React root mounts the window-root layer in `App.tsx` (`QuickCaptureApp`). |
| `<Dialog>` / `<AlertDialog>` (shadcn) | N/A | No `<Dialog>` or `<AlertDialog>` primitive lives in `components/ui/`; no consumer mounts one. The doc-comment on `FocusLayer` records the prescription so when a dialog component is added later the wrapping is unambiguous. |
| `<Popover>` consumers (date editor, color-palette editor, group selector) | **Not a layer** | Single-control overlay — the chooser owns its own arrow-key handling for option traversal; arrow keys never need to escape back to the field beneath. |
| `<DropdownMenu>` (`ui/dropdown-menu.tsx`) | **Not a layer** | Native menu focus loop, dismisses on outside click and Escape. |
| Context menus | **Not a layer** | Native menu through Tauri `context-menu-command`; no in-window React overlay to wrap. |
| Tooltips | **Not a layer** | Non-interactive. |
| Slide panel (`slide-panel.tsx`) | **Not a layer** at the slide-panel level | The slide-panel chrome is rendered inside the inspector layer via `inspectors-container.tsx`; the inspector layer covers it. |

### Wiring

- **`useCurrentLayerKey()` hook** lives in `kanban-app/ui/src/components/focus-layer.tsx` (alongside the strict variant `useOptionalLayerKey`). It reads `FocusLayerContext` and throws when called outside any layer — every spatial scope must live inside a layer per the project contract.
- **Pattern (b) is in force**: `FocusLayer` accepts an optional `parentLayerKey` prop that explicitly overrides the ancestor context. Openers read their own layer key via `useCurrentLayerKey()` and pass it to the dialog; the dialog uses that key as the explicit parent so portals (which sever the React ancestor chain) cannot misroute the parent link.
- **Command palette wiring** in `app-shell.tsx`: `AppShell` reads `windowLayerKey = useCurrentLayerKey()` (the window-root layer key, since `App.tsx` wraps everything in `<FocusLayer name="window">`) and passes it to `<FocusLayer name={PALETTE_LAYER_NAME} parentLayerKey={windowLayerKey}>` wrapping `<CommandPalette>`. The layer mounts only when `paletteOpen` is true so popping the layer on close restores focus to the parent layer's `last_focused`.
- **Brand helpers**: `PALETTE_LAYER_NAME = asLayerName("palette")` is module-scoped in `app-shell.tsx` so re-renders never mint a fresh value — the `<FocusLayer>` push effect depends on `name`, and a fresh literal in JSX would force a tear-down / re-push cycle on every parent render. The `WINDOW_LAYER_NAME` constant in `App.tsx` follows the same pattern.

### Documentation

The doc-comment on `FocusLayer` in `focus-layer.tsx` codifies the layer-vs-not-a-layer policy authoritatively, listing every layered surface (window, inspector, palette, dialog) and every non-layer transient overlay (context menus, popovers/dropdowns/single-select menus, date pickers/calendar popovers) with the rationale for each. The "rule of thumb" — *if you would naturally write a `useEffect` that traps `keydown` for ArrowUp / ArrowDown / Tab to keep focus inside your overlay, you want a `<FocusLayer>`* — gives future contributors a clear test to apply when adding a new overlay.

### Tests

- **`focus-layer.test.tsx`** carries a dedicated "overlay scenarios" describe block with three tests:
  - `dialog opened from a window-rooted leaf has the window as its parent` — captures the opener's layer key and verifies the dialog's `<FocusLayer parentLayerKey={...}>` push records the window-root key as `parent`.
  - `dialog opened from inside an inspector panel has the inspector as its parent` — two-deep nesting (window → inspector); the dialog's `parent` is the inspector key, not the window root, even when the dialog renders in a tree-detached `render` to mirror a portaled overlay.
  - `palette layer's parent is the window root when opened from app-shell context` — mirrors the `AppShell → CommandPalette` topology and verifies pop-on-close.
- The strict `useCurrentLayerKey` describe block additionally pins the contract that the hook throws outside any layer and otherwise returns the value provided via `FocusLayerContext`.
- **`command-palette.test.tsx`** carries the existing palette behavioral tests (open/close, navigation, vim insert mode, scope chain sourcing, per-entity-type rendering); the layer wrapping is validated by `app-shell.test.tsx` and `focus-layer.test.tsx`.
- **Rust** — `swissarmyhammer-focus/tests/focus_registry.rs` carries `ancestors_of_layer_walks_parent_chain` (innermost-first walk: dialog → inspector → root) and `forest_with_two_windows_and_stacked_overlays` (multi-window isolation: each window's inspector + dialog is isolated via `window_label`). Plus `layer_stress_dialog_focused_sees_only_dialog_entries` in the nav-strategy tests verifies that nav inside a dialog layer sees only the dialog's entries.

### Verification

- `cd kanban-app/ui && npx vitest run` — 1553 tests pass across 143 files.
- `cargo test -p swissarmyhammer-focus` — all suites green (registry, traits, nav-strategy).