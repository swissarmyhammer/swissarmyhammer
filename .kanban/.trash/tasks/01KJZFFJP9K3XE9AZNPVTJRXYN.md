---
position_column: done
position_ordinal: l7
title: 'Fix doctest: display_as_table in swissarmyhammer-cli/src/commands/model/list.rs (line 377)'
---
Doctest fails: cannot find derive macro `Serialize` and cannot find function `display_as_table` in scope. Missing `use serde::Serialize;` and function not exported.