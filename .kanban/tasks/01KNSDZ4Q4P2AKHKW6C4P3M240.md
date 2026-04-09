---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffa080
title: '[nit] insert_opt has split trait bounds'
---
**File**: code-context-cli/src/ops.rs (insert_opt function)\n\n**What**: The function signature puts `V: Into<Value>` in angle brackets and `V: Clone` in a separate `where` clause. Both bounds apply to the same type parameter.\n\n**Suggestion**: Consolidate into a single where clause for consistency:\n```rust\nfn insert_opt<V>(args: &mut Map<String, Value>, key: &str, val: &Option<V>)\nwhere\n    V: Clone + Into<Value>,\n```" #review-finding