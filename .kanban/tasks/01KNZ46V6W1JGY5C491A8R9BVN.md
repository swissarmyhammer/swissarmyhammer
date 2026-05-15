---
assignees:
- wballard
depends_on:
- 01KNZ44E91F4NYAGZX13H0FDAJ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffa480
project: pill-via-cm6
title: Migrate BadgeListDisplay to MentionView
---
## What

Rewrite `BadgeListDisplay` to delegate to `MentionView` in list mode. The resolution logic (IDs → entities → slugs for reference fields; slug passthrough for computed tags) moves into `MentionView`'s input pipeline, and `BadgeListDisplay` becomes a thin adapter that converts the `value` array and the field definition into a list of `{entityType, id-or-slug}` items.

**Files to modify:**
- `kanban-app/ui/src/components/fields/displays/badge-list-display.tsx` — gut the inner rendering. Keep only:
  - Field-type inspection: read `field.type.entity` and `field.type.commit_display_names`
  - Empty-state handling (compact vs full "None")
  - Mode-based focus moniker / claim predicate passthrough (full mode gets per-item monikers + predicates for nav.left/nav.right; compact mode doesn't)
  - Construction of the `items` array passed to `<MentionView>`:
    - For reference fields: `items = values.map(id => ({ entityType: targetEntityType, id }))`
    - For computed tag fields: `items = values.map(slug => ({ entityType: targetEntityType, slug }))`
  - Pass `taskId` through to `MentionView` for the `task.untag` extra command when `isComputedSlug` is true

**No change to field registration** — `BadgeListDisplay` stays registered as the `badge-list` display under the existing key. Only its internal implementation swaps.

**Focus/navigation preservation:** The existing per-pill FocusScope chain with nav.left / nav.right claim predicates must keep working. `MentionView` list mode already renders one FocusScope per item (from the previous card). Pass `pillMonikers` and `pillClaimPredicates` through as `focusMonikers` / `claimWhens` arrays (plural variants), or fold them into the `items` array elements. Pick whichever keeps the `MentionView` API cleanest.

**Virtual tags consideration:** Look at `virtual-badge-list` in `definitions/virtual_tags.yaml` — this is a separate display type, not `badge-list`. Out of scope for this card; it has its own display component.

## Acceptance Criteria
- [ ] `BadgeListDisplay` imports `MentionView`, not `MentionPill`
- [ ] Reference fields (`depends_on`, etc.) render the same visible pill text as before, but via the CM6 widget
- [ ] Computed tag fields render the same visible pill text as before
- [ ] Per-pill keyboard nav (nav.left/nav.right) still works in full mode
- [ ] Per-pill context menu still works, including the `task.untag` command for tags on a task
- [ ] Compact vs full mode behavioral differences preserved
- [ ] Empty state unchanged (`-` compact, italic `None` full)

## Tests
- [ ] Update `kanban-app/ui/src/components/fields/displays/badge-list-display.test.tsx` — all existing tests should still pass after migration; update DOM assertions where the markup differs (widget span inside a CM6 contentDOM vs. direct span)
- [ ] Update `kanban-app/ui/src/components/fields/displays/badge-list-nav.test.tsx` — confirm navigation tests still pass
- [ ] Add a new test: depends_on list of three task IDs, render full mode, assert each pill shows the task's clipped display name
- [ ] Run: `bun test badge-list-display badge-list-nav` — all pass
- [ ] Smoke: `bun run dev`, open a card with depends_on and tags, confirm visual parity

## Workflow
- Use `/tdd` — update tests to describe the new rendering, watch them fail, then implement the migration. Prioritize nav tests since those exercise the trickiest integration (FocusScope + keyboard).