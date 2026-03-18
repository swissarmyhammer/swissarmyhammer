---
position_column: done
position_ordinal: ffff8280
title: '[WARNING] view CRUD commands write changelog before persisting -- TOCTOU data inconsistency'
---
In swissarmyhammer-kanban-app/src/commands.rs, the view.create (line 704), view.update (line 734), and view.delete (line 759) commands all log to the changelog BEFORE calling write_view/delete_view. If the subsequent persistence call fails, the changelog records an operation that never completed. The changelog write should happen AFTER the mutation succeeds, or both should be done atomically. #warning