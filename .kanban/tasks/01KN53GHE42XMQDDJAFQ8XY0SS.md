---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffae80
title: 'store.rs serialize: body field trailing newline not normalized -- potential round-trip divergence'
---
swissarmyhammer-entity/src/store.rs:120 and :151\n\nOn serialize (line 120): `format!(\"---\\n{}---\\n{}\", frontmatter_yaml, body)` -- the body is appended as-is after the closing `---\\n`.\n\nOn deserialize (line 151): `parts[2].strip_prefix('\\n').unwrap_or(parts[2])` -- the leading newline after the closing `---` is stripped.\n\nThis means if the body does NOT end with a newline, serialize produces `---\\ncontent` and deserialize reads it back as `content` (correct). But if the body DOES end with a newline, serialize produces `---\\ncontent\\n` and deserialize reads it back as `content\\n` (correct). The issue is that serde_yaml_ng::to_string always appends a trailing `\\n` to the frontmatter YAML, so the closing delimiter is `---\\n` and the body immediately follows. This appears correct.\n\nHowever, there is no test verifying bodies with trailing newlines, empty bodies, or bodies that are just whitespace. These are realistic edge cases for markdown content.\n\nSuggestion: Add tests for (a) empty body, (b) body with trailing newline, (c) body that is just whitespace. Severity: nit. #review-finding