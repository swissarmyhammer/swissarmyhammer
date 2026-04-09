---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffc780
title: TrackedStore trait is not sealed -- public and implementable by downstream crates
---
swissarmyhammer-store/src/store.rs\n\nThe `TrackedStore` trait is public and not sealed. Adding a new required method (e.g. `store_version()`) would be a breaking change for downstream implementors. Currently implementations exist in swissarmyhammer-entity (EntityTypeStore) and swissarmyhammer-perspectives (PerspectiveStore).\n\nSuggestion: Seal the trait with a private `Sealed` supertrait if external implementations are not intended. #review-finding