---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffbf80
title: '[nit] SKILL.md metadata block uses non-standard key name'
---
**File**: builtin/skills/code-context/SKILL.md\n\n**What**: The frontmatter uses `metadata:` with nested `author` and `version` fields. This is fine for the skill format, but the `version` field uses `{{version}}` template syntax. The `format_skill_md` function in `skill.rs` does not emit a `metadata:` block -- it only emits `name`, `description`, and `allowed-tools`. So the `metadata` block from the SKILL.md source is lost during deployment.\n\n**Suggestion**: Verify whether the `metadata` block is intentionally stripped during deployment or if `format_skill_md` should preserve it. If it should be preserved, update `format_skill_md` to include metadata fields." #review-finding