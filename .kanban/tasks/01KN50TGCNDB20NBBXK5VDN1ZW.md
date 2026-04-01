---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffbd80
title: '[warning] sanitize_filename strips all leading dots but allows empty results'
---
**File**: `swissarmyhammer-entity/src/io.rs:377-384`\n\n**What**: `sanitize_filename` strips path separators, null bytes, and leading dots. But if the input is all dots (e.g. `\"...\"`) or empty after stripping, it returns an empty string. An empty stored filename would produce `\"{ulid}-\"` which is a valid-looking but semantically broken filename.\n\n**Why**: An empty filename after sanitization produces a stored file named `01ABC-` with no extension. This would cause MIME detection to fail and could confuse downstream logic. Edge case, but the function is a security boundary.\n\n**Suggestion**: Add a fallback to `\"unnamed\"` when the sanitized result is empty, similar to how `copy_attachment` already falls back to `\"unnamed\"` for missing filenames:\n```rust\nlet safe = sanitized.trim_start_matches('.').to_string();\nif safe.is_empty() { \"unnamed\".to_string() } else { safe }\n```\nAlso add a unit test for this edge case." #review-finding