---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffb180
title: Display virtual tags in task card header below regular tags
---
## What

Virtual tags (READY, BLOCKED, BLOCKING) are computed by Rust and already present in each task entity's `fields.virtual_tags` as `string[]` — but they are invisible because `virtual_tags.yaml` has `display: none` and no `section`. They need to render as colored pills in the task card header, below the regular tags row.

**Approach — Option B (dedicated display, no entity store coupling):**

Virtual tags aren't real tag entities in the store, so `MentionPill` can't resolve their color/description. Instead, add a lightweight `VirtualTagBadges` component that reads the `virtual_tags` string array from the entity fields and renders pills using the known metadata (slug→color mapping: READY=`#0e8a16`, BLOCKED=`#e36209`, BLOCKING=`#d73a4a`).

**Steps:**
1. Update `swissarmyhammer-kanban/builtin/definitions/virtual_tags.yaml` — change `section` to `header` (keep `display` as a new value like `virtual-badge-list` to distinguish from regular badge-list)
2. Create `kanban-app/ui/src/components/fields/displays/virtual-tag-display.tsx` — a display component that maps virtual tag slugs to colored pills. Use the same pill styling as `MentionPill` but resolve color from a static map instead of the entity store. Include tooltip with the tag description.
3. Register the `virtual-badge-list` display adapter in `kanban-app/ui/src/components/fields/registrations/` (follow the pattern in `multi-select.tsx` for `badge-list`)
4. Ensure field ordering in `EntityCard` (`kanban-app/ui/src/components/entity-card.tsx`) places `virtual_tags` after `tags` — the YAML field order or an explicit sort may be needed.

**Virtual tag metadata (from Rust `DEFAULT_REGISTRY` in `virtual_tags.rs`):**
- `READY` — color `0e8a16` (green), description "Task has no unmet dependencies"  
- `BLOCKED` — color `e36209` (orange), description "Task has at least one unmet dependency"
- `BLOCKING` — color `d73a4a` (red), description "Other tasks depend on this one"

**Files to modify:**
- `swissarmyhammer-kanban/builtin/definitions/virtual_tags.yaml` — add `section: header`, set `display: virtual-badge-list`
- `kanban-app/ui/src/components/fields/displays/virtual-tag-display.tsx` — new display component
- `kanban-app/ui/src/components/fields/registrations/` — register the new display adapter

## Acceptance Criteria
- [ ] Virtual tags (READY, BLOCKED, BLOCKING) appear as colored pill badges in the task card header
- [ ] Virtual tags render below the regular tags row (field ordering in the header section)
- [ ] Each pill uses the correct color: READY=green, BLOCKED=orange, BLOCKING=red
- [ ] Hovering a virtual tag pill shows a tooltip with its description
- [ ] Tasks with no virtual tags show nothing (no empty row)

## Tests
- [ ] Component test in `kanban-app/ui/src/components/fields/displays/__tests__/virtual-tag-display.test.tsx`: given `virtual_tags: ["READY", "BLOCKING"]`, renders two pills with correct colors and text
- [ ] Component test: given `virtual_tags: []` or undefined, renders nothing
- [ ] `pnpm test` passes with no regressions

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.