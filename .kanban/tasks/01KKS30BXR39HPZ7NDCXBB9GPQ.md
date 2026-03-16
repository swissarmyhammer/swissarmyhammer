---
assignees:
- claude-code
position_column: done
position_ordinal: fffff580
title: 'WARNING: list_tools uses tracing::warn with debug-scaffolding emoji in filtering proxy'
---
swissarmyhammer-mcp-proxy/src/proxy.rs:165-168\n\nThe `list_tools` handler logs at `WARN` level with a debug emoji:\n```rust\ntracing::warn!(\n    \"🔍 PROXY list_tools called - filtering from {}\",\n    self.upstream_url\n);\n```\nThis is not changed by the PR (it pre-dates the branch) but the PR touched this function block as part of downgrading `call_tool` from INFO to DEBUG. A warn-level log for a routine operation (listing tools on every connect/refresh) will pollute production logs and fire alerts in any monitoring setup. The emoji is also a sign this was intended as temporary debug output.\n\nSuggestion: Change `tracing::warn!` to `tracing::debug!` and remove the emoji. This was apparently overlooked while fixing `call_tool` in the same impl block.\n\nVerification: Search for `tracing::warn` in proxy.rs; confirm the list_tools call changes level and the emoji is gone.", 
<parameter name="tags">["review-finding"] #review-finding