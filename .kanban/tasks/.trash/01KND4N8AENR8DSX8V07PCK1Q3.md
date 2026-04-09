---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8880'
title: 'VT-9: Visual distinction for virtual tag pills'
---
## What

Virtual tag pills should be visually distinguishable from regular tags — dashed border signals "computed, not editable".

With Option B, virtual tags are NOT in the entity store. `MentionPill` resolves them via the virtual tag metadata fallback (from VT-3). The resolution path itself tells us whether a tag is virtual: if it was resolved from the metadata map rather than the entity store, it's virtual.

**Files to modify:**
- `kanban-app/ui/src/components/mention-pill.tsx`:
  - `MentionPill` already attempts entity store resolution. After VT-3, it falls back to virtual tag metadata.
  - Track which resolution path succeeded. If virtual metadata: set `isVirtual = true`.
  - Pass `isVirtual` prop to `MentionPillInner`.
  - In `MentionPillInner`, when `isVirtual`: use `border-style: dashed`, add CSS class `mention-pill-virtual`.

This is clean because the virtual/non-virtual distinction emerges from the resolution path — no flag on the entity, no extra field check.

## Acceptance Criteria
- [ ] Virtual tag pills render with dashed border
- [ ] Regular tag pills still render with solid border (no regression)
- [ ] `mention-pill-virtual` CSS class present on virtual tag pills for test targeting
- [ ] Resolution fallback from VT-3 drives the distinction (no hardcoded slug list)

## Tests
- [ ] `kanban-app/ui/src/components/mention-pill.test.tsx` — test that tag resolved from virtual metadata renders with dashed border / virtual class
- [ ] `kanban-app/ui/src/components/mention-pill.test.tsx` — test that entity-store tag renders with solid border
- [ ] `pnpm --filter kanban-app-ui test` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#virtual-tags