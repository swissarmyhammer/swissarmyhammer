---
position_column: done
position_ordinal: '8880'
title: HashMap iteration order makes extract_text non-deterministic
---
swissarmyhammer-entity-search/src/semantic.rs:15-24\n\n`extract_text()` iterates `entity.fields` (a HashMap) and joins values with spaces. HashMap iteration order is non-deterministic, so the same entity can produce different concatenated text across runs. This means embeddings built on one run may not match the text order on another, causing subtle inconsistency in semantic search scoring.\n\nSuggestion: Sort fields by key before concatenating, or use a BTreeMap-backed iteration:\n```rust\nlet mut keys: Vec<&String> = entity.fields.keys().collect();\nkeys.sort();\nfor key in keys {\n    if let Some(s) = entity.fields[key].as_str() { ... }\n}\n```