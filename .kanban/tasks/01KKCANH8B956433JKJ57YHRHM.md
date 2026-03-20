---
depends_on:
- 01KKCADZV71JJH01V5GVEPQTAX
position_column: done
position_ordinal: ffffffffffc480
title: 'STATUSLINE-M13: kanban module'
---
## What
Implement the `kanban` module that shows a progress bar of done/total tasks from the kanban board.

**File**: `swissarmyhammer-statusline/src/modules/kanban.rs`

**Source data**: `swissarmyhammer-kanban` board API — task counts per column

**Default format**: `📋 [$bar] $done/$total`

**Config**:
```yaml
kanban:
  style: "yellow"
  bar_width: 6
  format: "📋 [$bar] $done/$total"
  thresholds:
    low: { below: 25, style: "red" }
    medium: { below: 75, style: "yellow" }
    high: { below: 101, style: "green" }
```

**Variables**: `$bar` (filled/empty blocks), `$done` (count in done column), `$total` (all tasks)

**Example output**: `📋 [██░░░░] 2/7` (red), `📋 [████░░] 5/7` (green)

**API usage**:
- Open `.kanban` directory in workspace root
- Query board for column task counts
- Sum "done" column count vs total across all columns
- Bar rendering same as context_bar (█ for filled, ░ for empty)

**Color thresholds**: Same pattern as context_bar — percentage of done/total determines color.

## Acceptance Criteria
- [ ] Uses `swissarmyhammer-kanban` library API, NOT shell commands
- [ ] Reads task counts from kanban board in workspace
- [ ] Renders progress bar with configurable width
- [ ] Color thresholds apply based on done percentage
- [ ] Hidden when no `.kanban` directory found
- [ ] Format string supports `$bar`, `$done`, `$total` variables

## Tests
- [ ] Unit test: bar rendering at various done/total ratios
- [ ] Unit test: threshold color selection
- [ ] Unit test: hidden when no kanban board
- [ ] `cargo test -p swissarmyhammer-statusline` passes #statusline