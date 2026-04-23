---
assignees:
- claude-code
depends_on:
- 01KPX6E0QPNRWZTQXGXX2MBEMV
position_column: todo
position_ordinal: e780
project: spatial-nav
title: Enter = drill-into-container — board column header drills to first card, fields keep Enter=edit
---
## Reopened (2026-04-23) — previously closed but not actually working

The task was moved to `done` but user manual verification shows Enter on a focused board column header does NOT drill to the first card. The implementation either shipped incomplete, never landed against the real column-header FocusScope, or was broken by a subsequent change.

### Verification-first requirement

Before claiming this task is done again:

1. **Live manual check:** open the running app, click a board column header (any column with at least one card), press Enter. Focus MUST move to the first card's moniker — verify in the macOS unified log for `[FocusScope] focus → task:...`, AND verify the focus bar visually moves to the first card.
2. **Capture dump:** run `__spatial_dump` (debug Tauri command) while on a column header; paste a `## Diagnostic Evidence (YYYY-MM-DD HH:MM)` block into this task description showing:
   - The column header's focused key and moniker
   - That a `column.enterChildren.<id>` command is present in its scope chain (or wherever it lives)
   - The first card's moniker that the binding should resolve to
3. **If the command never appears in the scope chain** — the binding was never actually wired into the column header's FocusScope `commands` prop. Grep `kanban-app/ui/src/components/column-view.tsx` for `column.enterChildren` or any `FocusScope commands=` pattern on the header. If absent, the implementation didn't ship.
4. **If the command appears but execute doesn't fire on Enter** — the dispatch pipeline isn't routing. Check whether the parent scope's `grid.editEnter` / etc. is shadowing Enter at the board layer. Child-scope Enter should win, per `command-scope.tsx` shadow rules.
5. **If execute fires but focus doesn't move** — `setFocus(firstCardMoniker)` was called but the moniker doesn't match any registered scope. Confirm the first card's moniker matches what `spatial_register` recorded for it. Tie to the broader task `01KPVDA8NYFFQ8R1D2G9YEATJ3` if card registrations are still not happening.

### Evidence gate

**Do not close this task again without:** (a) a captured diagnostic dump in the task description showing the binding IS wired and reachable, (b) a passing vitest-browser test that dispatches Enter on a real column header FocusScope and asserts focus moves to the first card's moniker (not just mocks it), (c) a live-app screenshot-equivalent log capture showing the `[FocusScope] focus → ...` line after the Enter keypress.

The test must fail on the current `done`-marked-but-broken code. If the test passes on current code but the user still reports manual breakage, that's itself the diagnostic — the test is mocking something real would catch, and needs to exercise more of the stack.

---

## Original task body (preserved below)

## What

Now that `ui.inspect` has been moved to Space (task `01KPX6E0QPNRWZTQXGXX2MBEMV`), Enter is free to become the universal "activate / drill into the focused scope" verb. This task ships the first container-drill binding:

- **Board column header + Enter → focus the first card in that column**

Enter's semantics, stated uniformly across every scope type:

| Scope | What "activate" means | Already bound / added by this task |
|---|---|---|
| LeftNav button | switch to that view | already bound (keep) |
| Perspective tab | switch to that perspective | already bound (keep) |
| **Column header (board)** | **focus first card in the column** | **added by this task** |
| Grid cell | enter edit mode for this cell | already bound — Enter on an editable field IS a drill-in (into the editor) |
| Inspector field row | enter edit mode for this field | already bound — same reasoning |
| Card | drill into sub-parts (tag pills, title, etc.) — but only if they're registered as spatial children; otherwise no-op | defer to a follow-up if needed |
| Row selector | no drill target (the row IS the entity); Enter here either does nothing OR stays as "drill to first cell of this row" | decide during implementation based on what feels right; if ambiguous, leave unbound |

The unifying rule: **Enter on an editable field starts editing (drill into the editor). Enter on a container moves focus into the container's first child. Enter on a tab-like scope switches to it. No site ever uses Enter for inspect — that's Space's job.**

This task is specifically for the column-header case. Other container types opt in later via the same pattern.

### Why "Enter = edit" and "Enter = drill" are the same verb

A text field is a "container" for text. Pressing Enter on it means "go in — I want to interact with the contents." A column is a container for cards. Pressing Enter means "go in — focus the first card." A LeftNav button is an activator for a view. Pressing Enter means "switch to it." All three are activation — the specific action varies by what the scope represents.

This framing is important because it tells the implementer: do not special-case grid cells or inspector fields. Their existing `grid.editEnter` / `inspector.editEnter` bindings are the "edit" flavor of the same Enter rule. Leave them alone.

### Implementation

`kanban-app/ui/src/components/column-view.tsx` — find the column header's `FocusScope` (likely around `ColumnHeader` component, ~line 349-383 per earlier research). Add a `commands` array with an Enter binding:

```tsx
const firstCardMoniker = column.tasks[0]?.moniker;
const commands: CommandDef[] = useMemo(() => {
  if (!firstCardMoniker) return [];  // empty column → no drill-in
  return [{
    id: `column.enterChildren.${column.id}`,
    name: `Focus first card in ${column.name}`,
    keys: { vim: "Enter", cua: "Enter", emacs: "Enter" },
    execute: () => setFocus(firstCardMoniker),
    contextMenu: false,
  }];
}, [firstCardMoniker, column.id, column.name, setFocus]);
```

Wire `commands={commands}` on the column header's `FocusScope`.

For an empty column, no Enter binding — Enter falls through scope chain to any outer handler, or does nothing. Do not invent a "do something clever on empty" behavior.

### Fields keep Enter = edit (no change)

Verify these remain bound to Enter:

- `kanban-app/ui/src/components/grid-view.tsx` — `grid.edit` (cua: Enter) and `grid.editEnter` (vim: Enter). Keep.
- `kanban-app/ui/src/components/inspector-focus-bridge.tsx` — `inspector.edit` (cua: Enter) and `inspector.editEnter` (vim: Enter). Keep.

These ARE the "Enter = drill" rule applied to editable fields. Starting edit mode is drilling into the field's content.

### Out of scope

- Popping out to parent (some Shift+Enter or Escape-to-parent). Geometric nav (k upward from first card → column header) already handles this.
- Card Enter → drill into sub-scopes (tag pills). File a follow-up if cards have navigable sub-parts and you want this.
- Grid column headers (data-table) Enter → drill into first body cell. Same pattern, but follow-up task — data-table headers are a separate site and may want the same behavior later.
- Generalizing to an "Entity.activate" command that dispatches drill-in for every container type via a Rust handler. Premature abstraction — start with the one site, copy the pattern to follow-ups as needed.

### Supersedes

Task `01KPX5VD4W25K1ATD6BVPHCMPX` is **superseded** by this task. Close that one as duplicate/obsolete when this lands — its recommendation (Enter on column header drills to first card) is now the uncontested design because Enter is no longer polymorphic with inspect.

### Depends on

Task `01KPX6E0QPNRWZTQXGXX2MBEMV` (Enter → Space for inspect) must land first. Otherwise pressing Enter on a card simultaneously tries to inspect (old binding) and drill (this task's binding — though cards aren't touched by this specific task, the shadow resolution would still misbehave on column headers if Enter inspect bindings exist on parent scopes).

## Acceptance Criteria

- [ ] Pressing Enter on a focused board column header with at least one card moves focus to the first card in that column's task list
- [ ] Pressing Enter on a focused column header with zero cards leaves focus unchanged (no crash, no noisy error, no misleading emission)
- [ ] `k` (up) from the first card of a column navigates back to the column header via normal beam test
- [ ] Grid cell + Enter still enters edit mode (unchanged)
- [ ] Inspector field row + Enter still enters edit mode (unchanged)
- [ ] LeftNav button + Enter still switches view (unchanged)
- [ ] Perspective tab + Enter still switches perspective (unchanged)
- [ ] Card + Enter does NOT open the inspector (that's Space's job after the dependency lands)
- [ ] Row selector + Enter does NOT open the inspector (same reason)
- [ ] Task `01KPX5VD4W25K1ATD6BVPHCMPX` is closed as superseded
- [ ] All existing tests green
- [ ] **Diagnostic dump captured** in the description per the "Reopened" section above — proves the binding is wired AND reachable in the live app

## Tests

- [ ] Add a vitest-browser test in `kanban-app/ui/src/components/column-view.test.tsx` (or a dedicated `spatial-nav-column-drill.test.tsx` fixture): render a board column with 3 cards, focus the column header, dispatch Enter, assert focus moves to the first card's moniker
- [ ] Add an empty-column regression test: column with zero cards, focus header, dispatch Enter, assert focus is unchanged and no `setFocus` call is made
- [ ] Add cross-scope regressions: Enter on a grid cell still enters edit mode; Enter on an inspector field still enters edit mode; Enter on a LeftNav button still switches view
- [ ] **New regression test that would have caught this false-close:** a test that exercises the real dispatch path end-to-end (keypress → createKeyHandler → dispatch → column.enterChildren → setFocus) against a live FocusScope, not a mock. Must fail on the current `done`-marked code.
- [ ] Run `cd kanban-app/ui && npm test` — green
- [ ] Manual: load a board, click a column header, press Enter → focus moves to the first card. Press `k` → focus returns to the header.

## Workflow

- Use `/tdd`. Write the "Enter on column header drills to first card" test first — make it a test that would have caught the current false-close.
- Confirm task `01KPX6E0QPNRWZTQXGXX2MBEMV` has landed before starting — this task's behavior assumes Enter is no longer bound to inspect.
- Check if the column header currently has any other Enter binding (e.g. an old rename shortcut). If yes, the rename binding moves to F2; document the decision in the commit message.
- Do NOT add drill-in bindings to cards, row selectors, grid column headers, or any other scope. One site only. Follow-ups can copy the pattern.
- **Do not mark this task done without the diagnostic dump in the description proving live verification.** The task was closed once without working; the only reliable gate against a repeat is captured evidence.
