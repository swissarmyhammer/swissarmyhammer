---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffff8f80
title: 'Bug: All views show the board icon (LayoutGrid / 4 squares) except the board view'
---
## What
Reported by user: every view in the left-nav shows the same "4 squares" icon (which makes no sense for non-board views) — only the board view shows a sensible icon.

The "4 squares" is lucide's `LayoutGrid`, the fallback icon in `viewIcon` (`apps/kanban-app/ui/src/components/left-nav.tsx`).

## Root Cause (confirmed)
Two layers, both fixed:

1. **Corrupted on-disk view overrides clobbered builtin metadata.** The builtin view definitions (`crates/swissarmyhammer-kanban/builtin/views/*.yaml`) DO declare valid lucide icons (`kanban`, `folder`, `tag`, `table`). But this board's runtime files `.kanban/views/01JMVIEW0000000000{PGRID0,TGGRD0,TGRID0}.yaml` had been rewritten to degenerate `{id, name: '', kind: unknown}` (the residue of a partial `set view` with all-default fields — `SetView` defaults every field; follow-up card 01KTSK3239VGVWXW8M6F4HBHJ9). `build_views_context` merges builtins then local files with wholesale per-id override, so the degenerate local files stripped the grid views of name/icon/kind and the frontend received icon-less views → LayoutGrid fallback everywhere except the (intact) board view.
2. **`viewIcon` borrowed `view.kind` as an icon name** (`view.icon ?? view.kind`) — relying on kind strings accidentally matching lucide component names (root-cause candidate 2 in this card). Removed: the icon now comes only from view metadata.

## Fix
- `ViewDef::validate()` (views crate): a view definition with an empty name is degenerate. `from_yaml_sources` and `load_views` now skip degenerate definitions like parse failures (so a corrupted local override cannot shadow a builtin), and `write_view` refuses to persist them (closing the path that wrote the junk files). `InvalidViewDef` error variant maps to MCP `invalid_params`.
- Repaired the three corrupted `.kanban/views/*.yaml` from their builtin sources.
- Frontend: extracted `viewIcon` to `apps/kanban-app/ui/src/components/view-icon.ts` mirroring the `fieldIcon` pattern — a dumb metadata lookup returning `LucideIcon | null`; the left-nav applies the single documented `LayoutGrid` fallback. No kind→icon map anywhere.

## Acceptance Criteria
- [x] Each view kind renders a distinct, sensible icon (board=kanban, tasks-grid=table, tags-grid=tag, projects-grid=folder; guard test asserts pairwise-distinct).
- [x] The icon is supplied by view/service metadata (each built-in ViewDef declares a valid lucide `icon`); `viewIcon` is a dumb lookup + single documented fallback, with NO hardcoded kind→icon map in the component.
- [x] Root cause identified (corrupted local overrides clobbering builtin metadata + the invalid `kind`→lucide assumption).

## Tests
- [x] Unit test for `viewIcon` (`view-icon.node.test.ts`): valid icon resolves, kebab-case resolves, unknown/empty icon → null (caller fallback), and the kind is never borrowed as an icon name.
- [x] Guard test (`builtin-view-icons.guard.node.test.ts`): every built-in view definition declares an `icon` that resolves to a real lucide component, icons pairwise-distinct (red-verified by mutating a builtin icon).
- [x] Regression tests failing before the fix, passing after: `from_yaml_sources_degenerate_override_does_not_clobber_builtin`, `load_views_skips_degenerate_definition`, `write_view_rejects_empty_name` (Rust); "never borrows the view kind as an icon name" (TS).

## Workflow
- Use `/tdd` — failing test first, then fix. #bug

## Review Findings (2026-06-10 15:30)

### Warnings
- [x] `apps/kanban-app/ui/src/components/view-icon.ts:10` — `kebabToPascal` is a verbatim copy of the one in `fields/field-icon.ts`, and the `viewIcon` body is identical to `fieldIcon` except for the parameter type (the docstring even says "Mirrors fieldIcon"). Two deliberate copies of the same pure logic will drift the next time the lucide lookup needs a tweak. Extract a shared `iconByName(name: string | null | undefined): LucideIcon | null` (e.g. `src/lib/icon-name.ts`) and have both `fieldIcon` and `viewIcon` delegate to it. Note: the copy count did not increase with this change (left-nav previously held the second copy inline), so this is a missed unification, not a regression. **Fixed: extracted `src/lib/icon-name.ts::iconByName` (TDD'd via `icon-name.test.ts`); `fieldIcon` and `viewIcon` are now one-line delegators with unchanged public APIs. A third copy in `entity-icon.tsx` was discovered and filed as a follow-up card (out of this finding's scope).**