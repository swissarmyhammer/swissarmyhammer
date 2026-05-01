---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffa80
title: '[Low] Potential panic in entity type capitalization on empty string'
---
**File**: `kanban-app/src/commands.rs` line 1286\n\n**Issue**: The expression `&resolved[..1].to_uppercase()` will panic with an index-out-of-bounds if `resolved` is an empty string. While the current fallback returns `\"entity\"` (never empty), the code path through `clipboard_type.as_deref()` could yield `Some(\"\")` if someone sets an empty clipboard entity type, bypassing the `unwrap_or(\"entity\")` guard.\n\n**Severity**: Low (robustness)\n**Layer**: Functionality/Correctness\n\n**Fix**: Guard with `if resolved.is_empty() { \"Entity\" } else { format!(...) }`, matching the pattern already used in `resolve_name_template()` in `scope_commands.rs`."