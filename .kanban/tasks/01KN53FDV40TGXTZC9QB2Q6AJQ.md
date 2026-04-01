---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffe380
title: 'store.rs: is_computed does linear scan of field_defs on every field during serialize'
---
swissarmyhammer-entity/src/store.rs:65-69\n\nis_computed() iterates all field_defs for every field in the entity during serialization. With F fields and D field_defs, this is O(F * D) per serialize call. For typical entity sizes (5-20 fields, 5-20 defs) this is negligible, but if entity types grow or serialize is called in a hot loop, a HashSet of computed field names built once in the constructor would be O(F) per serialize.\n\nSuggestion: Pre-compute a `HashSet<String>` of computed field names in `EntityTypeStore::new()` and use `contains()` in serialize. Low priority since current entity sizes are small. Severity: nit. #review-finding