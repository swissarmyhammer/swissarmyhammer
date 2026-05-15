---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffc80
title: 'Fix clippy: unused import `parse_moniker` in swissarmyhammer-commands'
---
Clippy with `-D warnings` fails due to an unused import.\n\nFile: `swissarmyhammer-commands/src/ui_state.rs` line 5\n\n```\nuse crate::context::parse_moniker;\n```\n\nThis import is unused and must be removed to pass `cargo clippy --workspace -- -D warnings`.\n\n- [ ] Remove the unused `use crate::context::parse_moniker;` line from `swissarmyhammer-commands/src/ui_state.rs`\n- [ ] Verify `cargo clippy --workspace -- -D warnings` passes #test-failure