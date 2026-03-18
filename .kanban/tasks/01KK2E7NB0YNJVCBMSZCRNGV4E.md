---
position_column: done
position_ordinal: ffffcb80
title: Column sort command
---
Implement grid.sort command that sorts entity list by field. Sort state is ephemeral (not persisted).

**Update `ui/src/hooks/use-grid.ts`:**
- [ ] Add `sortStack: Array<{field: string, direction: "asc" | "desc"}>` to state
- [ ] Add `setSort(stack)` function

**Sort comparators (in grid-view or utility):**
- [ ] `alphanumeric` — locale-aware string compare
- [ ] `lexical` — simple string compare
- [ ] `option-order` — compare by SelectOption.order from field definition
- [ ] `datetime` — date string compare
- [ ] `numeric` — number compare

**Update `ui/src/components/data-table.tsx`:**
- [ ] Column headers show sort indicator (arrow up/down) when sorted
- [ ] Clicking column header toggles sort: asc → desc → none

**Update `ui/src/components/grid-view.tsx`:**
- [ ] `grid.sort` command registered in grid command scope
- [ ] Sort entities before passing to DataTable using sort stack + comparators
- [ ] Multi-field sort stack works (primary + secondary)

- [ ] All 5 sort kinds implemented and tested
- [ ] Sort indicator visible in header
- [ ] Click-to-sort works on column headers