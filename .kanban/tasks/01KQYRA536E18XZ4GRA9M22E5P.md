---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffa080
project: spatial-nav
title: 'spatial-nav: vim G / gg (nav.first / nav.last) doesn''t move from a focused leaf — should jump to first/last sibling'
---
## Bug

In the inspector and on cards with multiple vertically-stacked fields, vim `gg` (`nav.first`) and `Shift+G` (`nav.last`) do not move focus to the first/last field. They stay put.

## Root cause

`swissarmyhammer-focus/src/navigate.rs::edge_command` enumerates children of the **focused scope** (`children_of(focused.fq)`), not children of the focused scope's **parent_zone**. When the focused scope is a leaf (an inspector field, a card field), the children set is empty → returns `focused_fq` → stay-put.

Vim G / gg semantics expect "first / last sibling in my container" — i.e., enumerate `children_of(focused.parent_zone)` and pick topmost-leftmost / bottommost-rightmost.

## Dispatch path (already correct, no changes here)

- `gg` → vim sequence table → `nav.first` command
- `Shift+G` → keybinding → `nav.last` command
- Both call `actions.navigate(focusedFq, "first"|"last")` → IPC `spatial_navigate(focusedFq, Direction::First|Last, snapshot)` → kernel `BeamNavStrategy::next` → `edge_command`

## Fix

In `swissarmyhammer-focus/src/navigate.rs::edge_command`:

```rust
let children: Vec<NavScopeRef<'_>> = match focused.parent_zone {
    Some(parent) => snapshot.children_of(parent).collect(),
    None => snapshot.children_of(&focused.fq).collect(),
};
```

Preserve the drill-into-children fallback when `parent_zone` is `None` (focus on a layer root) so existing `first_picks_topmost_child` / `last_picks_bottom_rightmost_child` tests still pass — they start focus on `/L/parent` which has no parent_zone.

## Tests

1. **Kernel regression** in `swissarmyhammer-focus/src/navigate.rs::tests`: focus on `/L/parent/c2` with siblings `c1`, `c2`, `c3` (all sharing parent_zone `/L/parent`); assert `First` returns `c1`, `Last` returns `c3`.
2. **Browser test** in `kanban-app/ui/src/components/entity-inspector.field-vertical-nav.browser.test.tsx` (or new sister file): mount inspector, focus the middle field, fire `gg`, assert focus lands on first field; fire `Shift+G`, assert focus lands on last field.
3. **Card test**: similar coverage in `entity-card.in-zone-nav.spatial.test.tsx` — focus middle field, G/gg jumps to first/last.
4. Existing `first_picks_topmost_child`, `last_picks_bottom_rightmost_child`, `deprecated_row_directions_alias_first_last` should continue to pass (they exercise the `parent_zone == None` fallback path).

## Acceptance criteria

- `cargo test -p swissarmyhammer-focus` green
- `pnpm -C kanban-app/ui test` green
- `gg` / `Shift+G` work on inspector fields and card fields

## Files

- `swissarmyhammer-focus/src/navigate.rs` — `edge_command` fix + new test
- `kanban-app/ui/src/components/entity-inspector.field-vertical-nav.browser.test.tsx` — add G/gg coverage
- `kanban-app/ui/src/components/entity-card.in-zone-nav.spatial.test.tsx` — add G/gg coverage
#stateless-nav

## Review Findings (2026-05-06 06:59)

### Warnings
- [x] `kanban-app/ui/src/components/entity-card.in-zone-nav.spatial.test.tsx:306-141` — Both new card tests (`vim gg from card.inspect:{id}...` and `vim Shift+G from the title field zone...`) bypass the keybinding/keymap layer. Their own comments admit it: `"we drive spatial_navigate directly through the harness"` and `"The keybinding plumbing is exercised by the separate inspector test"`. That makes these tests effectively kernel-from-JS exercises, not end-to-end card coverage of `gg`/`Shift+G`. The inspector test correctly flips `keymap_mode: "vim"` and fires `g`/`g` keypresses; the card tests should mirror that pattern (set vim mode, dispatch `g`/`g` and `Shift+G` keys) so the card path also covers vim sequence resolution → `nav.first`/`nav.last` execution → IPC dispatch. Otherwise a regression in the card-path keybinding wiring would slip past CI.

### Nits
- [x] `kanban-app/ui/src/components/entity-card.in-zone-nav.spatial.test.tsx:310-311` — Indentation is broken in the `beforeEach` body. Line 310 is indented with 8 spaces (`        installLegacyScopeIpcBridge(mockInvoke);`) and line 311 is flush-left (`harness = setupSpatialHarness(...)`), inside a 2-space-indented file. Run prettier on the file (`pnpm -C kanban-app/ui exec prettier -w src/components/entity-card.in-zone-nav.spatial.test.tsx`).

## Resolution (2026-05-06 07:04)

### Warning fix
Rewrote both vim card tests to mirror the inspector test's end-to-end pattern:
- Re-install the spatial harness with `vimInvokeImpl` so `get_ui_state` returns `keymap_mode: "vim"` — AppShell's global key handler resolves `gg` to `nav.first` and `Shift+G` to `nav.last`.
- Dispatch real keypresses with `fireEvent.keyDown(document, { key: "g", code: "KeyG" })` (twice for `gg`) and `{ key: "G", code: "KeyG", shiftKey: true }` for Shift+G — the same shape the inspector test uses.
- Added `spatialNavigateCalls()` helper and assert that the keypresses dispatch `spatial_navigate` with `direction: "first"` / `"last"` and the right `focusedFq`. This closes the wiring gap the reviewer flagged: a regression in the card-path keybinding wiring (vim sequence resolution, `nav.first`/`nav.last` execute closure, IPC dispatch) now surfaces in CI.
- Kept the `data-focused` belt-and-braces assertion since the harness still actually performs the navigation.

### Nit fix
Re-indented the `beforeEach` body and ran prettier on the whole file. Prettier normalised the indentation and tidied a few unrelated import-line breaks.

### Verification
- `pnpm vitest run src/components/entity-card.in-zone-nav.spatial.test.tsx` — 4 tests pass.
- `pnpm vitest run src/components/entity-inspector.field-vertical-nav.browser.test.tsx` — 6 tests pass (no regression).
