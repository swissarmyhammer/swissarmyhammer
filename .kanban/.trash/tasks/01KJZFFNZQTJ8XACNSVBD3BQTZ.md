---
position_column: done
position_ordinal: l9
title: 'Fix doctest: display_as_yaml in swissarmyhammer-cli/src/commands/model/list.rs (line 487)'
---
Doctest fails: cannot find derive macro `Serialize` and cannot find function `display_as_yaml` in scope. Missing `use serde::Serialize;` and function not exported.