---
assignees:
- claude-code
position_column: doing
position_ordinal: '8280'
title: VirtualTagStrategy trait is not sealed -- downstream crates can implement it
---
swissarmyhammer-kanban/src/virtual_tags.rs\n\nThe `VirtualTagStrategy` trait is public and not sealed. Adding a new required method in the future would be a breaking change for any downstream crate that implements it.\n\nCurrently all implementations are internal (ReadyStrategy, BlockedStrategy, BlockingStrategy), so sealing the trait would have no impact on existing consumers.\n\nSuggestion: Add a sealed supertrait pattern (private module with a `Sealed` trait) to prevent external implementations. This preserves the ability to add methods without breaking semver. #review-finding