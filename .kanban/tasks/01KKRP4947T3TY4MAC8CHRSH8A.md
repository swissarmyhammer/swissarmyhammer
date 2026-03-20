---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffd080
title: 'avp-common/types/common.rs: CommonInput session_id/transcript_path silently default to empty string'
---
avp-common/src/types/common.rs (CommonInput struct, new `#[serde(default)]` on session_id and transcript_path)

The diff adds `#[serde(default)]` to `session_id` and `transcript_path` to support session-less events (InstructionsLoaded, WorktreeCreate, WorktreeRemove). The comment correctly identifies the motivation.

However, all existing callers that pattern-match or log `session_id` will now silently work with `""` for those events. Code that passes `session_id` to downstream systems (e.g., SQLite `session_id` column in heb, log formatters) will store/display empty strings with no indication they represent a session-less event.

Suggestion: use `Option<String>` for both fields, or introduce a sentinel value (`"<none>"`) that is more obviously distinct from a real session ID. If `Option<String>` is chosen, the downstream SQLite schema and logging code should handle `None` explicitly. #review-finding