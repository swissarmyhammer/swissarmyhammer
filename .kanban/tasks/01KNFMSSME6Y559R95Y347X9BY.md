---
assignees:
- claude-code
position_column: todo
position_ordinal: 9b80
title: EntityContext.write changelog is no longer written when StoreHandle is registered
---
swissarmyhammer-entity/src/context.rs -- write() method\n\nWhen a `StoreHandle` is registered, `EntityContext::write()` delegates to `sh.write()` and skips the legacy `io::write_entity` path. The legacy path included writing a per-entity changelog entry (via `changelog::ChangeEntry`). The `StoreHandle` path writes its own changelog (per-item JSONL alongside the data file), but the format is different (it stores unified diffs, not field-level changes).\n\nThis means the activity log that the kanban app shows (which reads the per-entity JSONL changelog for field-level changes) will stop getting entries for entity types that have a registered store. The watcher/dispatch_command path partially compensates by emitting events, but the persistent changelog is lost.\n\nSuggestion: After the `sh.write()` call, also append a `ChangeEntry` to the legacy per-entity changelog so that activity history continues to work. Alternatively, migrate the activity view to use the store's changelog format. #review-finding