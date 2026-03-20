---
depends_on:
- 01KKCADZV71JJH01V5GVEPQTAX
position_column: done
position_ordinal: ffffffffffc580
title: 'STATUSLINE-M14: index module'
---
## What
Implement the `index` module that shows code-context indexing progress.

**File**: `swissarmyhammer-statusline/src/modules/index.rs`

**Source data**: `swissarmyhammer-code-context` — `CodeContextWorkspace::open()` as Reader, then `get_status(conn)`

**Default format**: `idx $percent%`

**Config**:
```yaml
index:
  style: "blue"
  format: "idx $percent%"
  show_when_complete: false
```

**Variables**: `$percent` (integer 0-100 from `StatusReport.ts_indexed_percent`), `$total` (total files), `$indexed` (indexed files)

**Example output**: `idx 85%` (visible during indexing), hidden when 100% and `show_when_complete: false`

**API usage**:
- `CodeContextWorkspace::open(workspace_root)` — opens as Reader if leader exists
- `crate::ops::status::get_status(ws.db())` — returns `StatusReport` with `ts_indexed_percent`
- Gracefully handle: no `.code-context/` directory, no database, lock errors

## Acceptance Criteria
- [ ] Uses `swissarmyhammer-code-context` library API
- [ ] Opens workspace as Reader (read-only)
- [ ] Shows indexing percentage from StatusReport
- [ ] Hidden when complete and `show_when_complete: false`
- [ ] Hidden when no `.code-context/` directory exists
- [ ] Format string supports `$percent`, `$total`, `$indexed` variables

## Tests
- [ ] Unit test: formats percentage correctly
- [ ] Unit test: hidden when 100% and show_when_complete is false
- [ ] Unit test: shown when 100% and show_when_complete is true
- [ ] `cargo test -p swissarmyhammer-statusline` passes #statusline