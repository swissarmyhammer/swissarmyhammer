---
assignees:
- claude-code
position_column: todo
position_ordinal: '8680'
title: 'warning: `test_helpers.rs` uses `#![cfg(test)]` inner attribute — prevents use in integration tests'
---
swissarmyhammer-tools/src/mcp/tools/shell/test_helpers.rs (line 6)\n\nThe file begins with `#![cfg(test)]` which means the entire module is compiled only during `cargo test`. However `mod.rs` declares it as:\n```rust\n#[cfg(test)]\npub(crate) mod test_helpers;\n```\n\nThe `#[cfg(test)]` on the `mod` declaration in `mod.rs` is redundant with the inner `#![cfg(test)]`. Having both is harmless but the inner `#![cfg(test)]` on the file itself prevents the helpers from being used in `tests/` integration test files (which are not in the `#[cfg(test)]` compilation unit). If the helpers are ever needed for integration tests, this will be a problem.\n\nSuggestion: Remove the `#![cfg(test)]` inner attribute from `test_helpers.rs` and rely solely on the `#[cfg(test)]` gate on the `mod` declaration in `mod.rs`. This is the conventional Rust pattern." #review-finding