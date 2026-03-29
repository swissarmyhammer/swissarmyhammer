---
depends_on:
- 01KKCADZV71JJH01V5GVEPQTAX
position_column: done
position_ordinal: ffffffffffb480
title: 'STATUSLINE-M4: cost module'
---
## What
Implement the `cost` module that displays the session's total cost in USD.

**File**: `swissarmyhammer-statusline/src/modules/cost.rs`

**Source data**: `cost.total_cost_usd` from stdin JSON

**Default format**: `$$amount`

**Config**:
```yaml
cost:
  style: "dim"
  format: "$$amount"
  hide_zero: true
```

**Variables**: `$amount` (formatted as `0.42`)

**Example output**: `$0.42`

## Acceptance Criteria
- [ ] Module reads `cost.total_cost_usd` from parsed input
- [ ] Formats cost with 2 decimal places
- [ ] Hidden when `hide_zero: true` and cost is 0 or absent
- [ ] Format string supports `$amount` variable

## Tests
- [ ] Unit test: formats `0.42` as `$0.42`
- [ ] Unit test: hidden when cost is 0 and hide_zero is true
- [ ] Unit test: shown when cost is 0 and hide_zero is false
- [ ] `cargo test -p swissarmyhammer-statusline` passes #statusline