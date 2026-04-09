---
assignees:
- claude-code
depends_on:
- 01KN2Q6HQN1PYDEQ6XYEMCQSSP
position_column: done
position_ordinal: ffffffffffffffffae80
title: 'PERSP-6: MCP schema verification and integration test'
---
## What

Verify that perspective operations are properly exposed through the MCP tool pipeline. The kanban MCP tool dispatches through `execute_operation()`, so adding operations to dispatch (PERSP-5) should automatically make them available. This card is about verification and adding examples.

**`swissarmyhammer-kanban/src/schema.rs`:**
- Verify perspective ops appear in generated MCP schema `op` enum
- Add perspective-specific examples to `generate_kanban_examples()`

**Integration test:**
- Full lifecycle through the dispatch path: add perspective → get perspective → update perspective → list perspectives → delete perspective
- Verify JSON output shapes match expectations
- Verify changelog entries are produced for mutations

## Acceptance Criteria
- [x] MCP schema `op` enum includes "add perspective", "get perspective", "list perspectives", "update perspective", "delete perspective"
- [x] Schema examples include at least one perspective example
- [x] Full lifecycle integration test passes through dispatch
- [x] Changelog has entries for add, update, delete operations

## Tests
- [x] `test_schema_includes_perspective_ops` — check op enum values
- [x] `test_schema_has_perspective_examples`
- [x] `test_perspective_lifecycle_integration` — full add/get/update/list/delete through dispatch with changelog verification
- [x] Run: `cargo test -p swissarmyhammer-kanban perspective`