---
position_column: done
position_ordinal: ffffffffff8880
title: store.rs replay() has duplicated query logic
---
**store.rs:70-107**\n\nThe filtered and unfiltered branches of `replay()` duplicate the query_map + deserialization logic. Only the SQL WHERE clause differs.\n\n**Suggestion**: Build the query string conditionally and use a single deserialization loop. Example:\n```rust\nlet (sql, params) = if let Some(cat) = category {\n    (\"...WHERE seq > ?1 AND category = ?2...\", vec![...])  \n} else {\n    (\"...WHERE seq > ?1...\", vec![...])\n};\n```\n\n**Verify**: `cargo test -p heb` passes.