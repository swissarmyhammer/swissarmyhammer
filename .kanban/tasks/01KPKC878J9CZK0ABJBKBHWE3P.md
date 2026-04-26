---
assignees:
- claude-code
position_column: todo
position_ordinal: '9480'
title: 'clippy: 3 unnecessary_sort_by violations in swissarmyhammer-code-context'
---
Pre-existing clippy errors surfaced by `cargo clippy --workspace --all-targets -- -D warnings`. Not regressions from kanban project_commands deletion or cut transactional safety tests — files unchanged from merge-base.

Locations:
- swissarmyhammer-code-context/src/ops/get_symbol.rs:226 — `suffix.sort_by(|a, b| b.1.cmp(&a.1))` → use `sort_by_key(|b| std::cmp::Reverse(b.1))`
- swissarmyhammer-code-context/src/ops/get_symbol.rs:259 — `fuzzy.sort_by(|a, b| b.1.cmp(&a.1))` → same fix
- swissarmyhammer-code-context/src/ops/search_symbol.rs:118 — `matches.sort_by(|a, b| b.score.cmp(&a.score))` → same fix

Acceptance Criteria:
- All three sort_by calls converted to sort_by_key with std::cmp::Reverse
- `cargo clippy -p swissarmyhammer-code-context --all-targets -- -D warnings` passes

Tests: clippy is the test — re-run after change. #test-failure