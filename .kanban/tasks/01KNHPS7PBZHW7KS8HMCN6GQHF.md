---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffec80
title: Add moniker field to all Entity literals in 28 test files
---
Add `moniker: "type:id"` to all Entity literals in test files. The moniker format is `entity_type + ":" + id`. 28 files need fixing under kanban-app/ui/src/. Verify with `npx tsc --noEmit 2>&1 | grep "moniker.*missing"` returning zero results.