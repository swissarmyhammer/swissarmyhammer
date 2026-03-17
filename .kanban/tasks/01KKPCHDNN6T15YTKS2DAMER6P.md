---
position_column: done
position_ordinal: z00
title: HookEvent variants with empty session_id — consider Option<String>
---
`hookable_agent.rs` — to_base_json() for InstructionsLoaded, WorktreeCreate, WorktreeRemove

These variants hardcode `"session_id": ""` in their JSON output because the events don't naturally have a session context. This works for deserialization (CommonInput.session_id is a String), but an empty string is semantically wrong — it implies a session exists.

Consider either:
1. Making session_id an `Option<String>` in these HookEvent variants and omitting it from JSON when None
2. Making CommonInput.session_id `#[serde(default)]` so it can be absent

This is minor since these are forward-compat and not fired today, but the empty string will be confusing in logs.

**Severity**: nit #review-finding