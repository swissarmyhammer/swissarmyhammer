---
title: Add unwrap_or guard in get_entity_schema serde_json::to_value
position:
  column: todo
  ordinal: e2
---
**File:** `swissarmyhammer-kanban-app/src/commands.rs`, line 393\n\n**What:** Inside `get_entity_schema`, the field serialization uses `.map(|f| serde_json::to_value(f).unwrap_or(Value::Null))`. If a FieldDef fails to serialize (which should not normally happen but could if a FieldDef has an unserializable value), the error is silently swallowed and replaced with `null`. The caller then receives a `fields` array with a null entry, with no indication something went wrong.\n\n**Why:** Silent null injection in schema data is confusing for the frontend. It would be better to propagate the error so the caller knows the schema is malformed, or at minimum log a warning.\n\n**Suggestion:** Either propagate the error with `map_err(|e| e.to_string())?` instead of `unwrap_or(Value::Null)`, or log when a null fallback is used. Given this is infrastructure code, propagating the error is cleaner.\n\n- [ ] Replace `unwrap_or(Value::Null)` with error propagation or explicit logging\n- [ ] Verify the frontend handles error responses from `get_entity_schema` gracefully" #warning