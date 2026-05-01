---
assignees:
- claude-code
depends_on:
- 01KMTTV2PMJQ3GM2ED1B441N8Y
position_column: done
position_ordinal: ffffffffffeb80
title: 'E2E tests: git merge with kanban merge drivers'
---
## What\nEnd-to-end integration tests proving the full git merge workflow using the `kanban` binary's merge drivers.\n\n**File:** `kanban-cli/tests/merge_e2e.rs`\n\nSame test scenarios as the deleted `swissarmyhammer-cli/tests/merge_e2e.rs` but using `kanban merge` instead of `sah merge`:\n- Use `env!(\"CARGO_BIN_EXE_kanban\")` for the binary path\n- Register drivers as `kanban merge jsonl %O %A %B` etc.\n- All tests `#[ignore]` (requires built binary + git)\n\n**Test scenarios:**\n- JSONL: disjoint appends merge clean; same-id-different-content conflicts\n- YAML: non-overlapping field changes merge clean; same-field conflict resolved by fallback\n- MD: frontmatter + body changes merge clean; overlapping body produces conflict markers\n\n## Tests\n- `cargo nextest run -p kanban-cli merge_e2e --run-ignored ignored-only`"}
</invoke>