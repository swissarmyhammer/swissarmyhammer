---
depends_on:
- 01KKCADZV71JJH01V5GVEPQTAX
position_column: done
position_ordinal: ffffffffffc080
title: 'STATUSLINE-M9: version module'
---
## What
Implement the `version` module that displays the Claude Code version.

**File**: `swissarmyhammer-statusline/src/modules/version.rs`

**Source data**: `version` from stdin JSON

**Default format**: `v$version`

**Config**:
```yaml
version:
  style: "dim"
  format: "v$version"
```

**Variables**: `$version` (version string)

**Example output**: `v1.0.23`

## Acceptance Criteria
- [ ] Module reads `version` from parsed input
- [ ] Hidden when version field is absent
- [ ] Format string supports `$version` variable

## Tests
- [ ] Unit test: formats version with "v" prefix
- [ ] Unit test: hidden when version absent
- [ ] `cargo test -p swissarmyhammer-statusline` passes #statusline