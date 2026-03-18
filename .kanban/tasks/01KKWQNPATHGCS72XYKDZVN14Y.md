---
assignees:
- claude-code
position_column: done
position_ordinal: fffffff580
title: '[warning] extract_paths still exported without deprecation signal'
---
avp-common/src/turn/mod.rs:19\n\n`extract_paths` is re-exported for backward compatibility but lacks any signal that callers should prefer `extract_tool_paths`. Internal code was updated, but external consumers won't know to switch.\n\nAdd `#[deprecated]` or a prominent doc comment noting the preferred alternative.\n\n**Verify**: grep for `extract_paths` usage across the workspace to confirm no external callers remain. #review-finding