---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffe080
title: 'warning: Old mod.rs module-level doc comment was dropped — useful architecture documentation lost'
---
swissarmyhammer-tools/src/mcp/tools/shell/mod.rs (line 1)\n\nThe original `mod.rs` had a detailed 30-line module-level doc comment describing the architecture, security considerations, process isolation model, and rate limiting. The new `mod.rs` has only a 9-line operational summary listing the six operations.\n\nThe deleted content included:\n- Architecture rationale (why tokio timeout, why process isolation)\n- Security considerations checklist\n- Rate limiting and resource monitoring notes\n\nThis documentation does not belong in any single operation module and there is now nowhere for it to live.\n\nSuggestion: Restore the architecture/security sections to `mod.rs` to preserve this institutional knowledge for future maintainers." #review-finding