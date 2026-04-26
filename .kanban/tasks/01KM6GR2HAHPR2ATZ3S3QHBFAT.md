---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffff9b80
title: 'Fix test_kanban_schema_has_all_operations: update expected op count from 41 to 44'
---
Test at swissarmyhammer-cli/tests/integration/mcp_tools_registration.rs:218 hardcodes op count as 41, but 3 new operations were added: archive task, unarchive task, list archived. Update expected count to 44.