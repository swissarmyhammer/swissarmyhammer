---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffa380
title: 'store.rs: flatten_into is duplicated verbatim from io.rs'
---
swissarmyhammer-entity/src/store.rs:184-193 vs swissarmyhammer-entity/src/io.rs:286-295\n\nThe `flatten_into` function is an exact copy of the one in io.rs. Both have identical signature, logic, and doc comments. This is a DRY violation -- if the flattening logic changes (e.g., to support deeper nesting or array flattening), both copies must be updated in lockstep.\n\nSuggestion: Extract `flatten_into` into a shared location (e.g., `entity.rs` as a method on Entity, or a `util.rs` module) and have both io.rs and store.rs call it. Alternatively, if io.rs is being deprecated in favor of store.rs, leave it and remove the io.rs copy when io.rs is retired. #review-finding