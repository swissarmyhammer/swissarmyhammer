---
depends_on:
- 01KKHF6QWJ0N4CS7X9JBMR1FXA
position_column: done
position_ordinal: ffca80
title: 'SEM-3: Create git_types module (FileChange/FileStatus without git2)'
---
## What\nThe `git::types` module in sem-core defines `FileChange` and `FileStatus` — plain data structs with serde derives. These have NO git2 dependency. Copy them into `crates/swissarmyhammer-sem/src/git_types.rs` (flat module, no git:: nesting needed).\n\nDo NOT copy `git::bridge` — that's the git2-dependent code we're eliminating.\n\nThe `parser::differ::compute_semantic_diff` function takes `&[FileChange]` as input, so this type must be in the new crate.\n\nFiles:\n- `crates/swissarmyhammer-sem/src/git_types.rs` (new, ~40 lines from vendor/sem/crates/sem-core/src/git/types.rs)\n- Wire into lib.rs as `pub mod git_types`\n\n## Acceptance Criteria\n- [ ] `swissarmyhammer_sem::git_types::{FileChange, FileStatus}` compiles\n- [ ] These are the same types used by `compute_semantic_diff`\n- [ ] No git2 dependency\n\n## Tests\n- [ ] `cargo check -p swissarmyhammer-sem` passes