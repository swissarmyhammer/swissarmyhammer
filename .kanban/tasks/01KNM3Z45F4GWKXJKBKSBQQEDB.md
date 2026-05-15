---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffbe80
title: 'Fix failing test: cell_moniker_inspect_target_strips_field_suffix'
---
Test `scope_commands::tests::cell_moniker_inspect_target_strips_field_suffix` in swissarmyhammer-kanban fails.\n\nLocation: `swissarmyhammer-kanban/src/scope_commands.rs:1906`\n\nThe test expects that inspecting a cell moniker like `tag:tag-1.color` should resolve the inspect target to the base entity moniker `tag:tag-1`, but the code currently returns `tag:tag-1.color` (the full cell moniker with the field suffix still attached).\n\nAssertion:\n  left:  Some(\"tag:tag-1.color\")\n  right: Some(\"tag:tag-1\")\n\n#test-failure #test-failure