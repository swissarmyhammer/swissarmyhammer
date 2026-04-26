---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffce80
title: 'warning: validate_tool_names fails on first bad name only — misleading for multi-name input'
---
swissarmyhammer-cli/src/commands/tools/mod.rs:140-150\n\n`validate_tool_names` returns on the first unknown name. If the user runs `sah tools enable foo bar baz` and both `foo` and `baz` are unknown, only the error for `foo` is reported. The user must run the command again to discover `baz` is also unknown.\n\nThis is a minor UX issue but produces confusing iteration cycles:\n\n```\n$ sah tools enable foo bar baz\nUnknown tool 'foo'. Valid tools: ...\n$ sah tools enable bar baz   # after fixing foo\nUnknown tool 'baz'. Valid tools: ...\n```\n\nSuggestion: Collect all unknown names and report them together:\n\n```rust\nlet unknown: Vec<_> = names.iter().filter(|n| !KNOWN_TOOLS.contains(&n.as_str())).collect();\nif !unknown.is_empty() {\n    return Err(format!(\"Unknown tools: {}. Valid tools: {}\",\n        unknown.iter().map(|s| format!\"'{}'\", s)).collect::<Vec<_>>().join(\", \"),\n        KNOWN_TOOLS.join(\", \")));\n}\n```\n\nVerification: Update `test_validate_tool_names_unknown` to pass multiple bad names.\n\n#review-finding #review-finding