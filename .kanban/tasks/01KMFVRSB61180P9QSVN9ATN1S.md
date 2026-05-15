---
assignees:
- claude-code
attachments: []
depends_on:
- 01KMFVR6G48ZSGCZ0PWYJWYFPJ
position_column: done
position_ordinal: '80'
title: 'Frontend: Replace BoardProgress with Field-based percent_complete rendering'
---


## What

\n\n

The `BoardProgress` component (`kanban-app/ui/src/components/board-progress.tsx`) is a bespoke radial chart that reads from `board.summary.percent_complete`. This bypasses the metadata-driven Field system and doesn't react to `entity-field-changed` events for the board entity.\n\n**Fix**: Replace the custom `BoardProgress` with a `<Field>` rendering the board's `percent_complete` field from the entity store. The existing `entity-field-changed` handler in `App.tsx` already updates entities in the store, so once the backend card emits the event, the Field will re-render automatically.\n\nThe `ProgressDisplay` component (`kanban-app/ui/src/components/fields/displays/progress-display.tsx`) already renders `{ total, completed, percent }` objects — it may need a compact variant or the board can use a simpler number display.\n\n### Files to modify\n- `kanban-app/ui/src/components/nav-bar.tsx` — replace `<BoardProgress>` with `<Field>` for board percent_complete\n- `kanban-app/ui/src/components/board-progress.tsx` — delete or convert to a display component registered in the field system\n\n## Acceptance Criteria\n- [ ] Board progress is rendered as a Field, not a bespoke component\n- [ ] Progress updates reactively via entity store (no manual refresh needed)\n- [ ] Visual appearance is equivalent or better than current radial chart\n\n## Tests\n- [ ] Run: `cd kanban-app/ui && npm test` — no regressions\n- [ ] Manual: open inspector on board entity → percent_complete field visible"