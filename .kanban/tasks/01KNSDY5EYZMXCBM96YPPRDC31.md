---
assignees:
- claude-code
position_column: todo
position_ordinal: '8680'
title: '[warning] skill.rs format_skill_md hand-rolls YAML frontmatter'
---
**File**: code-context-cli/src/skill.rs (format_skill_md function)\n\n**What**: The `format_skill_md` function constructs YAML frontmatter by string concatenation with `push_str` and `format!`. This is fragile -- if any field value contains YAML special characters (colons, quotes, newlines), the output will be malformed YAML.\n\n**Why**: The skill description field is a free-text string that could contain colons or other YAML metacharacters. The current code does not quote or escape it. For example, a description containing `key: value` would produce invalid YAML.\n\n**Suggestion**: Use `serde_yaml_ng` (already a workspace dependency per commit 3990540a5) to serialize the frontmatter, or at minimum quote the description value. Alternatively, since the `Skill` struct already has serialization support, serialize the metadata portion properly.\n\n**Verify**: Create a skill with a description containing `: ` and verify the output parses as valid YAML." #review-finding