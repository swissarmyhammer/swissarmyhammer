---
position_column: done
position_ordinal: h8
title: Fix missing validate field in EntityDef struct literals in io.rs
---
The `EntityDef` struct gained a `validate: Option<String>` field, but the test helpers `task_entity_def()` and `tag_entity_def()` in `swissarmyhammer-entity/src/io.rs` (lines 322-337) were missing this field, causing compilation failure. Fixed by adding `validate: None` to both struct literals. #test-failure