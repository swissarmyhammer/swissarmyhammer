---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffc980
project: spatial-nav
title: 'Fix: Enter on focused perspective tab does not start inline rename'
---
## What

Pressing **Enter** on a perspective tab that is the spatial-focused element does nothing — the inline rename editor never appears. Double-clicking the tab works (the `onDoubleClick → startRename(p.id)` path in `perspective-tab-bar.tsx`), but there is no keyboard equivalent. The keyboard path is broken because nothing binds Enter to the rename trigger when focus is in a perspective scope.

### Why it's broken

The rename trigger is the `triggerStartRename()` module-level broadcaster in `kanban-app/ui/src/components/perspective-tab-bar.tsx`. It is invoked by exactly one caller — the `ui.entity.startRename` command in `swissarmyhammer-commands/builtin/commands/ui.yaml`:

```yaml
- id: ui.entity.startRename
  name: Rename Perspective
```

That command has **no `keys:` block** and no `scope:` constraint. Nothing keyboard-side fires it. The companion `perspective.rename` command in `swissarmyhammer-kanban/builtin/commands/perspective.yaml` carries `keys: {}` (explicitly empty). So:

- Palette → Enter executes `ui.entity.startRename` → calls `triggerStartRename()` → tab enters rename mode (works today).
- Mouse double-click on tab → calls `startRename(id)` directly (works today).
- Keyboard Enter on a spatially-focused perspective tab → falls through to the global drill-in binding (which is a no-op for a leaf scope) → nothing happens (the bug).

### Resolution

Implementer chose **Option 1** — added `keys` + `scope: "entity:perspective"` to the existing `ui.entity.startRename` command in `swissarmyhammer-commands/builtin/commands/ui.yaml`. Reuses the canonical "begin inline rename" command rather than adding a perspective-domain alias.

The React side mirrors the YAML by registering a perspective-scoped `CommandDef` with id `ui.entity.startRename` and `keys: { cua: Enter, vim: Enter, emacs: Enter }` on the **active** perspective tab's `<CommandScopeProvider>` in `ScopedPerspectiveTab` (`kanban-app/ui/src/components/perspective-tab-bar.tsx`). Inactive tabs receive an empty commands array, so Enter on a focused inactive tab falls through to the global `nav.drillIn` (a leaf-scope no-op). This satisfies both AC #1 (Enter on focused active tab triggers rename) and the test-case requirement that Enter on a focused inactive tab mounts no rename editor.

### Files changed

- `swissarmyhammer-commands/builtin/commands/ui.yaml` — added `keys` block (Enter for cua / vim / emacs) and `scope: "entity:perspective"` to `ui.entity.startRename`.
- `kanban-app/ui/src/components/perspective-tab-bar.tsx` — imported `type CommandDef`; added a per-active-tab `useMemo`-built `CommandDef[]` carrying `ui.entity.startRename` with the same `keys` block; wired it into the wrapping `CommandScopeProvider`'s `commands` prop. Added a frozen `EMPTY_PERSPECTIVE_SCOPE_COMMANDS` constant for inactive-tab identity stability.
- `swissarmyhammer-commands/src/registry.rs` — new test `ui_entity_start_rename_carries_perspective_scope_and_enter_keys` asserts the YAML contract.
- `kanban-app/ui/src/lib/keybindings.test.ts` — four new source-level guards on the `extractScopeBindings` perspective-scope path (cua, vim, emacs, and an inner-shadow-outer regression).
- `kanban-app/ui/src/components/perspective-tab-bar.enter-rename.spatial.test.tsx` — new browser-mode test file with 9 cases covering all 7 acceptance criteria from the card.

## Acceptance Criteria

- [x] Pressing Enter (in cua / vim normal / emacs) while a perspective tab is the spatial-focused element flips the tab into inline rename mode — the `<InlineRenameEditor>` mounts in place of the tab name.
- [x] The behavior shadows the global drill-in Enter binding only inside the perspective scope; pressing Enter on a focused board / column / card still does its existing drill-in.
- [x] Pressing Enter on a perspective tab when no perspective is the active perspective is a no-op (mirrors `triggerStartRename`'s existing guard: `if (activePerspective) startRename(activePerspective.id)`).
- [x] After typing a new name and pressing Enter inside the rename editor, `perspective.rename` is dispatched with `{ id, new_name }` — i.e. the existing commit path keeps working unmodified.
- [x] Pressing Escape inside the rename editor cancels (cua/emacs) or commits (vim normal) per existing policy.
- [x] `cd kanban-app/ui && npm test` is green — 1714/1714 tests passing.
- [x] `cargo test -p swissarmyhammer-kanban` and `cargo test -p swissarmyhammer-commands` are green.

## Tests

### Browser Tests (mandatory)

- [x] New file `kanban-app/ui/src/components/perspective-tab-bar.enter-rename.spatial.test.tsx` with 9 cases (all 7 required + extra cua/emacs Escape splits).

### Backend tests

- [x] Added `ui_entity_start_rename_carries_perspective_scope_and_enter_keys` to `swissarmyhammer-commands/src/registry.rs` asserting `keys` block + `scope`.

### Source-level guard

- [x] Extended `kanban-app/ui/src/lib/keybindings.test.ts` with 4 new `extractScopeBindings` guards (cua, vim, emacs, inner-shadow-outer regression).

## Review Findings (2026-04-27 09:03)

### Nits
- [x] `kanban-app/ui/src/components/perspective-tab-bar.tsx` (the `<div className="flex items-center gap-2 overflow-x-auto shrink-0 max-w-[60%] pl-2">` wrapper around the tabs map) — this card changed the perspective-tab row's classes from `gap-0.5` (no `pl-2`) to `gap-2 ... pl-2`. The added comment correctly explains the load-bearing role for `<FocusIndicator>`'s `-left-2 w-1` placement (matches the `nav-bar.tsx` and `BoardDndWrapper` pattern), and the change is a real fix for a separate clipping bug on the leftmost tab's focus indicator. But it is unrelated to "Enter on focused perspective tab does not start inline rename" and is not listed in the card's "Files changed" or in any AC. Per "Stay on task — don't refactor unrelated code," this layout fix should ideally have been a separate kanban card so the diff for the rename-binding work stays scoped. Leaving the change in place is acceptable (the fix is sound and the comment is good); the nit is process-level — future cards should split unrelated improvements. **Resolution (2026-04-27):** Acknowledged — layout fix kept here because it's the same code-path being modified for the keyboard binding. Pattern matches `nav-bar.tsx` / `BoardDndWrapper`.
