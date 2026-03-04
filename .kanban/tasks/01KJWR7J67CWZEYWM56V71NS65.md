---
position_column: done
position_ordinal: j1
title: Fix restore_entity_files to error on missing data file
---
Change the data file rename in restore_entity_files to return RestoreFromTrashFailed on NotFound instead of silently ignoring it