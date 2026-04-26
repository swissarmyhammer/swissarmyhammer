---
depends_on:
- 01KKCADZV71JJH01V5GVEPQTAX
position_column: done
position_ordinal: ffffffffffff8980
title: 'STATUSLINE-M5: session module'
---
## What
Implement the `session` module that displays a truncated session ID.

**File**: `swissarmyhammer-statusline/src/modules/session.rs`

**Source data**: `session_id` from stdin JSON

**Default format**: `$id`

**Config**:
```yaml
session:
  style: "dim"
  format: "$id"
  truncation_length: 8
```

**Variables**: `$id` (first N chars of session_id)

**Example output**: `a1b2c3d4`

## Acceptance Criteria
- [ ] Module reads `session_id` from parsed input
- [ ] Truncates to configurable length (default 8)
- [ ] Hidden when no session_id in input
- [ ] Format string supports `$id` variable

## Tests
- [ ] Unit test: truncates long session ID to 8 chars
- [ ] Unit test: short session ID displayed as-is
- [ ] Unit test: hidden when session_id absent
- [ ] `cargo test -p swissarmyhammer-statusline` passes #statusline