---
position_column: done
position_ordinal: ffa980
title: Rename boardDataToBoardData to a clearer name
---
In `kanban.ts` line 129, the function `boardDataToBoardData` converts `BoardDataResponse` to `BoardData`. The name is confusing (board data to board data?). Consider `parseBoardDataResponse` or `toBoardData`. #nit