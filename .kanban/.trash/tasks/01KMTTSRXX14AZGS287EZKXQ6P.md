---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: 'E2E tests: git merge with kanban merge drivers'
---
## What\nEnd-to-end integration tests proving the full git merge workflow using the `kanban` binary's merge drivers.\n\n**File:** `kanban-cli/tests/merge_e2e.rs`\n\nSame test scenarios as before but using `kanban merge` instead of `sah merge`:\n- Use `env!(\"CARGO_BIN_EXE_kanban\")` for the binary path\n- Register drivers as `kanban merge jsonl %O %A %B` etc.\n- All tests `#[ignore]` (requires built binary + git)\n\n**JSONL:** disjoint appends merge clean; same-id-different-content conflicts\n**YAML:** non-overlapping field changes merge clean; same-field conflict resolved by fallback\n**MD:** frontmatter + body changes merge clean; overlapping body → conflict markers\n\n## Tests\n- `cargo nextest run -p kanban-cli merge_e2e -- --ignored`"}
</invoke>