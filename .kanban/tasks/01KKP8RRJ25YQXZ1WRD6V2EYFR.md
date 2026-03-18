---
position_column: done
position_ordinal: e780
title: '[Info] Clean design: search_display_field fallback chain is well-structured'
---
Positive observation: The display field resolution in both the backend (commands.rs search_entities) and frontend (command-palette.tsx getDisplayField) follows the same fallback chain:\n\nsearch_display_field > mention_display_field > 'name' > 'title' > entity ID\n\nThis is consistent, well-documented, and every builtin entity YAML has search_display_field set explicitly. The `#[serde(default, skip_serializing_if)]` annotation on the new field ensures backward compatibility with existing YAML that does not include it.\n\nAll test fixtures across context.rs, validation.rs, derive.rs, io.rs, and derive_handlers.rs correctly add `search_display_field: None` to maintain exhaustive struct construction.\n\nNo action needed." #review-finding