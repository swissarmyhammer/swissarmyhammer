---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffe280
title: 'progress display: hide when total items is zero'
---
## What

The progress bar/display renders even when there are zero items (0/0). When there are no checkboxes or subtasks, the progress display should return null and take up no space on the card.

### Files to investigate
- Progress display component (likely in `kanban-app/ui/src/components/fields/displays/`)

## Acceptance Criteria
- [ ] Progress display returns null when total items is 0
- [ ] Cards without checkboxes show no progress bar
- [ ] Cards with checkboxes still show progress normally

## Tests
- [ ] Zero type errors
- [ ] Manual smoke test"