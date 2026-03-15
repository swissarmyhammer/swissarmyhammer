---
depends_on:
- 01KKHF8RWM7XNXMA3SDA4ATCPA
position_column: done
position_ordinal: e8
title: 'SEM-6: Remove vendor/sem and clean up workspace'
---
## What\nDelete the entire `vendor/sem/` directory and remove all references from the workspace.\n\nCleanup:\n- `rm -rf vendor/sem/`\n- Remove `vendor/sem/crates/sem-core` from workspace `members` in root `Cargo.toml`\n- Remove `sem-core = { path = ... }` from workspace `[dependencies]`\n- Remove any `.gitmodules` or git submodule references if applicable\n- Check no other crates depend on `sem-core`\n\nFiles:\n- `vendor/sem/` (delete entire directory)\n- `Cargo.toml` (remove workspace member + dependency)\n\n## Acceptance Criteria\n- [ ] `vendor/sem/` directory does not exist\n- [ ] `cargo check` succeeds for entire workspace\n- [ ] `cargo tree | grep git2` returns nothing (git2 fully eliminated)\n- [ ] `cargo release minor --dry-run` no longer hits libgit2-sys conflict\n\n## Tests\n- [ ] `cargo test --workspace` passes\n- [ ] `cargo tree | grep libgit2` returns nothing\n- [ ] `cargo release minor --dry-run` succeeds (or at least gets past the packaging step)