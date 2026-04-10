---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffa580
title: '[nit] FileWriterGuard is duplicated for the 5th time'
---
**File**: code-context-cli/src/main.rs (FileWriterGuard struct)\n\n**What**: `FileWriterGuard` is now copy-pasted in 5 crates: `code-context-cli`, `shelltool-cli`, `swissarmyhammer-cli`, `avp-cli`, and `swissarmyhammer-tools` (unified_server.rs). Each copy is identical.\n\n**Suggestion**: Extract into a shared utility in `swissarmyhammer-common` or a small `swissarmyhammer-logging` crate, then import from there. This is a pre-existing problem that this PR perpetuates." #review-finding