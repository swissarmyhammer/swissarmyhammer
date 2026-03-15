---
position_column: done
position_ordinal: l8
title: 'Fix doctest: display_as_json in swissarmyhammer-cli/src/commands/model/list.rs (line 430)'
---
Doctest fails: cannot find derive macro `Serialize` and cannot find function `display_as_json` in scope. Missing `use serde::Serialize;` and function not exported.