---
assignees:
- claude-code
position_column: todo
position_ordinal: ffdd80
title: Dead test helper profile_memory hides behind allow(dead_code)
---
`profile_memory` in `crates/swissarmyhammer-tools/tests/integration/file_tools_integrations/performance.rs` has no caller. It carries `#[allow(dead_code)]`, so the compiler does not report it.

Found while splitting the file for ^0fn6dbf. That card is a pure move, so the helper moved unchanged.

## What to do

- Decide whether the memory tests should call `profile_memory` or whether it should go.
- If it goes, delete it and its `#[allow(dead_code)]`.
- Sweep the same test tree for other `#[allow(dead_code)]` marks that hide a helper with no caller.

## Done when

- No helper in `file_tools_integrations/` is kept alive only by an `allow` mark.
- `cargo nextest run -p swissarmyhammer-tools` is green and `cargo clippy --workspace --all-targets -- -D warnings` is clean.

#tool-validators