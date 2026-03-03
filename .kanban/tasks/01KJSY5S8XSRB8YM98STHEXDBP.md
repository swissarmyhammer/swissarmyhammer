---
position_column: done
position_ordinal: h0
title: 'Non-atomic two-phase write: attachment created but task update may fail'
---
**Done.** Added comments documenting the two-phase write ordering rationale in both add.rs and delete.rs. Periodic cleanup deferred — orphans are now also cleaned up by DeleteTask (previous card).\n\n- [x] Document ordering rationale in add.rs and delete.rs\n- [x] Periodic cleanup — deferred, DeleteTask now handles the main case\n- [x] Code paths reviewed