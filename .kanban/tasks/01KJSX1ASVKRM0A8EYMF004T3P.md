---
position_column: done
position_ordinal: g5
title: Remove vestigial columns/swimlanes fields from Board struct
---
**Done.** Removed `columns: Vec<Column>` and `swimlanes: Vec<Swimlane>` fields from Board struct, along with the legacy deserialization test. Board is now just name + description.\n\n- [x] Remove columns and swimlanes fields\n- [x] Remove legacy migration test\n- [x] 211 tests pass, clippy clean