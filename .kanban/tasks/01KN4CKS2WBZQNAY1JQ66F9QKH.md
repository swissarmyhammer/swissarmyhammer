---
assignees:
- claude-code
depends_on:
- 01KN4CKKB9JHVD5RXTE4DKK7CJ
position_column: done
position_ordinal: ffffffffffffffffffb980
title: 'EXTRACT-6: Full workspace test verification'
---
## What

Final verification that the extraction didn't break anything.

Run:
- `cargo test -p swissarmyhammer-perspectives` — new crate standalone
- `cargo test -p swissarmyhammer-kanban` — all existing tests
- `cargo test -p swissarmyhammer-commands` — command tests
- `cargo clippy -p swissarmyhammer-perspectives -- -D warnings`
- `cargo clippy -p swissarmyhammer-kanban -- -D warnings`

Verify MCP round-trip still works: add → get → update → list → delete perspective through dispatch.

## Acceptance Criteria
- [ ] All tests pass across all 3 crates
- [ ] Clippy clean on both perspectives and kanban
- [ ] No dead code warnings from removed files

## Tests
- [ ] `cargo test --workspace` passes (or at minimum the 3 crates above)
- [ ] Clippy clean