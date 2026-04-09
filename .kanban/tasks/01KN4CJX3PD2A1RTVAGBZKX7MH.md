---
assignees:
- claude-code
depends_on:
- 01KN4CJJPSRQNEB17E6CQFF0PH
position_column: done
position_ordinal: ffffffffffffffffb880
title: 'EXTRACT-2: Move types.rs to new crate'
---
## What

Move `swissarmyhammer-kanban/src/perspective/types.rs` to `swissarmyhammer-perspectives/src/types.rs`.

No changes needed — types have zero kanban dependencies (only serde, serde_yaml_ng).

Update `swissarmyhammer-perspectives/src/lib.rs` with re-exports:
```rust
pub use types::{Perspective, PerspectiveFieldEntry, SortDirection, SortEntry};
```

Move the unit tests along with the file.

## Acceptance Criteria
- [x] `perspectives/src/types.rs` exists with all 4 types
- [x] All type tests pass in new crate
- [x] `cargo test -p swissarmyhammer-perspectives` passes

## Tests
- [x] All existing types tests pass in new location
- [x] `cargo check -p swissarmyhammer-perspectives`