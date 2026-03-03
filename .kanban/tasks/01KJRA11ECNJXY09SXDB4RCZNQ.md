---
position_column: done
position_ordinal: f6
title: GetColumn error type is generic EntityError, not ColumnNotFound
---
**Done.** Added `KanbanError::from_entity_error()` helper that maps `EntityError::NotFound` to the correct specific variant (TaskNotFound, ColumnNotFound, etc). Applied consistently across all entity get/update/delete operations.\n\nFiles changed: error.rs, column/{get,update,delete}.rs, swimlane/{get,update,delete}.rs, task/{get,update,delete}.rs, tag/{update,delete}.rs, actor/{get,update,delete}.rs\n\n- [x] Decide on error mapping strategy — helper method on KanbanError\n- [x] Apply consistently across get/update/delete for all entity types\n- [x] Verify no downstream callers broken (225 tests pass, clippy clean)