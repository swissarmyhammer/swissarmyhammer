---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffb980
title: StoreError Display messages use uppercase -- should be lowercase per convention
---
swissarmyhammer-store/src/error.rs\n\nSeveral `StoreError` variants have Display messages starting with uppercase: \"I/O error\", \"JSON error\", \"YAML error\", \"Store error\". The Rust review guidelines specify lowercase, no trailing punctuation for error Display messages.\n\nSuggestion: Change to lowercase: \"i/o error: {0}\", \"json error: {0}\", \"yaml error: {0}\", etc. #review-finding