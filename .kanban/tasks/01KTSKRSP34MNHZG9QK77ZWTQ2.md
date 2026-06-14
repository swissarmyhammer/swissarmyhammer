---
assignees:
- claude-code
position_column: todo
position_ordinal: e980
title: Unify entity-icon.tsx lucide lookup onto lib/icon-name.ts::iconByName
---
`apps/kanban-app/ui/src/components/entity-icon.tsx` carries its own verbatim `kebabToPascal` + lucide `icons` registry lookup — the same pure logic now centralized in `apps/kanban-app/ui/src/lib/icon-name.ts::iconByName` (extracted while working the review warning on 01KTCRY5W2BP7TYTHV4JB9CH8K, which scoped the unification to `fieldIcon`/`viewIcon` only).

## What
Replace the local helper + lookup in `EntityIcon` with `iconByName(iconName) ?? LayoutGrid`, keeping `EntityIcon`'s public API and `LayoutGrid` fallback behavior unchanged.

## Acceptance Criteria
- [ ] `entity-icon.tsx` no longer defines `kebabToPascal` or touches the lucide `icons` registry directly; it delegates to `iconByName`.
- [ ] Fallback behavior unchanged: missing schema icon or unresolvable name still renders `LayoutGrid`.
- [ ] Scoped vitest for entity-icon (and any consumers) + `tsc --noEmit` clean.

#refactor