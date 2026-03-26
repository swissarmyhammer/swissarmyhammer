---
position_column: done
position_ordinal: ffffc780
title: ViewKind::Grid + builtin YAML + view routing
---
Add Grid variant to backend ViewKind enum, create builtin tasks-grid.yaml, and wire frontend routing so clicking the grid icon renders a skeleton GridView component.

**Backend (Rust):**
- [ ] Add `Grid` to `ViewKind` enum in `swissarmyhammer-views/src/types.rs`
- [ ] Create `swissarmyhammer-kanban/builtin/views/tasks-grid.yaml` — id: `01JMVIEW0000000000TGRID`, name: `Tasks`, icon: `table-2`, kind: `grid`, entity_type: `task`, card_fields: `[title, tags, assignees, due, depends_on]`
- [ ] Update any test asserting builtin view count (currently checks board only)
- [ ] `cargo test` passes

**Frontend (TypeScript):**
- [ ] Add `table-2` / `grid` case to `viewIcon()` in `left-nav.tsx` → return `<Table2 />` from lucide-react
- [ ] In `ActiveViewRenderer` (App.tsx), add `else if (activeView.kind === "grid")` → render `<GridView entityType={activeView.entity_type} fieldNames={activeView.card_fields} />`
- [ ] Create skeleton `ui/src/components/grid-view.tsx` — renders placeholder div
- [ ] Left nav shows board + grid icons, clicking grid shows skeleton