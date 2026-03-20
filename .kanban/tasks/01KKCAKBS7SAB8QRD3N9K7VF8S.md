---
depends_on:
- 01KKCADZV71JJH01V5GVEPQTAX
position_column: done
position_ordinal: ffffffffffbd80
title: 'STATUSLINE-M6: vim_mode module'
---
## What
Implement the `vim_mode` module that displays the current vim mode when vim mode is active.

**File**: `swissarmyhammer-statusline/src/modules/vim_mode.rs`

**Source data**: `vim.mode` from stdin JSON

**Default format**: `$mode`

**Config**:
```yaml
vim_mode:
  style: "bold"
  format: "$mode"
```

**Variables**: `$mode` (NORMAL, INSERT, VISUAL, etc.)

**Example output**: `NORMAL`, `INSERT`

## Acceptance Criteria
- [ ] Module reads `vim.mode` from parsed input
- [ ] Hidden when vim object is absent (vim mode not enabled)
- [ ] Displays mode string as-is from input
- [ ] Format string supports `$mode` variable

## Tests
- [ ] Unit test: displays "NORMAL" when vim.mode is "NORMAL"
- [ ] Unit test: hidden when vim field absent
- [ ] `cargo test -p swissarmyhammer-statusline` passes #statusline