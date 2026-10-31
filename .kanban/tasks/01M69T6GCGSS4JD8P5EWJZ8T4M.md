---
assignees:
- claude-code
position_column: todo
position_ordinal: ffff9080
project: remove-shelltool
title: Remove the unused ShellConfig and the .shell gitignore entries
---
The shell crate is gone. `ShellConfig` in `crates/swissarmyhammer-directory/src/config.rs` has no user outside its own tests. Remove `ShellConfig`, its export in `lib.rs`, and its tests in `directory.rs`. Remove `ShellConfig` from the legacy dir list in `directory.rs` only if the list goal allows it. Then remove `**/.shell/` from `.gitignore`. Update the `.shell` text in `crates/swissarmyhammer-common/src/logging.rs` docs. Card ^vjmg9ae left these in place because `ShellConfig` can still write `.shell`. #remove-shell