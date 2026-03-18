---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: 'Inconsistent version source: use crate::VERSION vs env!(\"CARGO_PKG_VERSION\")'
---
**File:** `swissarmyhammer-tools/src/mcp/tools/skill/use_op.rs:60` vs `claude-agent/src/agent.rs:448`\n\n**What:** The two injection sites use different sources for the version string. `use_op.rs` uses `crate::VERSION` (a named constant defined in `lib.rs` via `env!(\"CARGO_PKG_VERSION\")`), while `agent.rs:load_sah_modes` uses `env!(\"CARGO_PKG_VERSION\")` directly inline.\n\n**Why:** Both expand to the same value at compile time and are therefore functionally equivalent today. However, mixing styles makes it harder to audit all version injection sites, and if the `VERSION` constant were ever changed (e.g., to append a build suffix), `agent.rs` would silently diverge. Consistency also aids grep-ability.\n\n**Suggestion:** Standardize on one approach. Prefer `env!(\"CARGO_PKG_VERSION\")` inline (simpler, zero-indirection) or introduce a shared version constant accessible from both crates. At minimum, both files in the same PR should use the same pattern.\n\n**Verification:** `grep -rn 'crate::VERSION\\|env!(\"CARGO_PKG_VERSION\")' claude-agent/src/ swissarmyhammer-tools/src/` shows exactly two injection sites using different patterns." #review-finding