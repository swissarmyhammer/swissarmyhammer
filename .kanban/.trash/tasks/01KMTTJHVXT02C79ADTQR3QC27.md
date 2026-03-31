---
assignees:
- claude-code
position_column: todo
position_ordinal: '8380'
title: '[NIT] thiserror listed in Cargo.toml but not used in swissarmyhammer-merge'
---
`swissarmyhammer-merge/Cargo.toml`\n\n`thiserror = { workspace = true }` is declared as a dependency, but `MergeConflict` in `lib.rs` implements `Display` and `Error` manually. The crate therefore pulls in an unused compile dependency.\n\nEither derive `#[derive(thiserror::Error)]` on `MergeConflict` (and any future error types) to take advantage of the dependency, or remove `thiserror` from `Cargo.toml` if manual impls are preferred." #review-finding