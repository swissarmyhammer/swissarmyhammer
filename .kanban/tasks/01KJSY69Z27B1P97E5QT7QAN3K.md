---
position_column: done
position_ordinal: h1
title: map_err discards original entity error context in get/delete/update
---
**Done.** Replaced error-discarding `map_err(|_| ...)` closures in attachment get/update with `map_err(KanbanError::from_entity_error)`. Updated from_entity_error catch-all to map unknown entity types to `NotFound { resource, id }` instead of generic EntityError wrapper.\n\n- [x] Replace closures in attachment get.rs, update.rs\n- [x] from_entity_error now handles all entity types including attachment\n- [x] 216 tests pass, clippy clean