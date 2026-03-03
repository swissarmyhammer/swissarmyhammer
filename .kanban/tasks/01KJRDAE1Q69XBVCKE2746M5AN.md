---
position_column: done
position_ordinal: g3
title: Default columns defined in three separate places
---
**Resolution:** Already done in commit 2316c7a8. Board::default_column_entities() is the single source of truth, called by both InitBoard and KanbanOperationProcessor. The local default_columns() in init.rs and inline tuple in processor.rs were removed.