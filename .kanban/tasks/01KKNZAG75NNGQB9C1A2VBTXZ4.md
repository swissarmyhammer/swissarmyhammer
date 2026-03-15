---
position_column: done
position_ordinal: '8980'
title: Unnecessary String allocation in fuzzy_search for string values
---
swissarmyhammer-entity-search/src/fuzzy.rs:32-34\n\n```rust\nlet text = match value.as_str() {\n    Some(s) => s.to_string(),\n    None => value.to_string(),\n};\n```\n\nThe `Some(s) => s.to_string()` branch allocates a new String from a `&str` only to pass it to `fuzzy_match` which accepts `&str`. The allocation is unnecessary for the common string-value case.\n\nSuggestion: Use `Cow<str>` or restructure to avoid the allocation:\n```rust\nuse std::borrow::Cow;\nlet text: Cow<str> = match value.as_str() {\n    Some(s) => Cow::Borrowed(s),\n    None => Cow::Owned(value.to_string()),\n};\n```