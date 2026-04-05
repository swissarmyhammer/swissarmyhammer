---
assignees:
- claude-code
position_column: todo
position_ordinal: ad80
title: PerspectiveError Display messages use uppercase
---
swissarmyhammer-perspectives/src/error.rs\n\nSame issue as the StoreError finding: Display messages start with uppercase (\"IO error\", \"YAML error\", \"JSON error\", \"Store error\").\n\nSuggestion: Change to lowercase per the Rust convention. #review-finding