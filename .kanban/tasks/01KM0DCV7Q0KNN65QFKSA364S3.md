---
assignees:
- claude-code
depends_on:
- 01KM0DC89WJ5A3ZAE4YYM1WATR
position_column: done
position_ordinal: ffffffffc080
title: Extract search_history operation
---
Move SearchHistory struct + impl Operation + handler into `execute/search_history/mod.rs`.
- Handler receives `(args, state)`, uses `state.search_handle()` then async search
- Move tests: `test_search_history_missing_query_returns_error`

**Verify**: `cargo nextest run -p swissarmyhammer-tools`