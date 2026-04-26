---
depends_on:
- 01KKCADZV71JJH01V5GVEPQTAX
position_column: done
position_ordinal: ffffffffffff8880
title: 'STATUSLINE-M3: context_bar module'
---
## What
Implement the `context_bar` module that renders a visual progress bar showing Claude Code's context window usage percentage.

**File**: `swissarmyhammer-statusline/src/modules/context_bar.rs`

**Source data**: `context_window.used_percentage` from stdin JSON

**Default format**: `[$bar] $percentage%`

**Config**:
```yaml
context_bar:
  bar_width: 10
  format: "[$bar] $percentage%"
  thresholds:
    low: { below: 50, style: "green" }
    medium: { below: 80, style: "yellow" }
    high: { below: 101, style: "red" }
```

**Variables**: `$bar` (filled/empty block chars), `$percentage` (integer 0-100)

**Example output**: `[████░░░░░░] 42%` (green), `[████████░░] 82%` (red)

Bar rendering: `█` for filled, `░` for empty. Color determined by threshold matching.

## Acceptance Criteria
- [ ] Module reads `context_window.used_percentage` from parsed input
- [ ] Bar renders with configurable width using block characters
- [ ] Color thresholds apply correct style based on percentage
- [ ] Hidden when no context_window data in input
- [ ] Format string supports `$bar` and `$percentage` variables

## Tests
- [ ] Unit test: bar rendering at 0%, 50%, 100% with width=10
- [ ] Unit test: threshold color selection (green < 50, yellow < 80, red >= 80)
- [ ] Unit test: hidden when input lacks context_window field
- [ ] `cargo test -p swissarmyhammer-statusline` passes #statusline