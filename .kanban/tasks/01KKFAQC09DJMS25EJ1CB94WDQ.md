---
position_column: done
position_ordinal: ffa580
title: '[warning] OwnedLspServerSpec::Display impl is redundant'
---
**Severity: warning**\n**File:** swissarmyhammer-lsp/src/types.rs:100-104\n\nThe `Display` impl for `OwnedLspServerSpec` formats as `{command} (command: {command})`, repeating the same value twice. This is likely a copy-paste error -- the second field was probably meant to be something else (e.g., the language IDs or project types). As-is it provides no useful information beyond just the command name." #review-finding