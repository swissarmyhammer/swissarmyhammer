---
depends_on:
- 01KKCADZV71JJH01V5GVEPQTAX
position_column: done
position_ordinal: ffffffffffbe80
title: 'STATUSLINE-M7: agent module'
---
## What
Implement the `agent` module that displays the active background agent name.

**File**: `swissarmyhammer-statusline/src/modules/agent.rs`

**Source data**: `agent.name` from stdin JSON

**Default format**: `🤖 $name`

**Config**:
```yaml
agent:
  style: "cyan"
  format: "🤖 $name"
```

**Variables**: `$name` (agent name string)

**Example output**: `🤖 code-review`

## Acceptance Criteria
- [ ] Module reads `agent.name` from parsed input
- [ ] Hidden when agent object is absent (no active agent)
- [ ] Format string supports `$name` variable

## Tests
- [ ] Unit test: displays agent name with emoji prefix
- [ ] Unit test: hidden when agent field absent
- [ ] `cargo test -p swissarmyhammer-statusline` passes #statusline