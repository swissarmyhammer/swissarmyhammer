---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffa180
title: 'store.rs serialize: io.rs format divergence -- no deterministic key ordering in io.rs means round-trip through both paths produces different output'
---
swissarmyhammer-entity/src/store.rs:104-118 vs swissarmyhammer-entity/src/io.rs:326-341\n\nstore.rs uses BTreeMap for deterministic alphabetical key ordering in serialized YAML. io.rs uses serde_json::Map (backed by IndexMap or BTreeMap depending on serde_json features, but populated from HashMap iteration which is unordered). This means the same entity serialized through io.rs and store.rs will produce different YAML key ordering.\n\nThis is not a bug in store.rs itself -- deterministic ordering is an improvement -- but it means files written by io.rs and then read+rewritten by EntityTypeStore will produce gratuitous diffs. If both code paths coexist during migration, this will cause spurious changelog entries.\n\nSuggestion: Either (a) port the BTreeMap ordering into io.rs as well so both paths are consistent, or (b) document that EntityTypeStore is the canonical path going forward and io.rs is deprecated. #review-finding