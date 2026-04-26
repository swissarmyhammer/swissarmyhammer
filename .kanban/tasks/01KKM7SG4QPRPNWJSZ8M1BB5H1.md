---
assignees:
- assistant
depends_on:
- 01KKM7RX9ZTK4VKMB6P9NPWHWW
position_column: done
position_ordinal: fffffff280
title: Update VirtualFileSystem to use XDG for user-level paths
---
## What
Update `VirtualFileSystem` in `swissarmyhammer-directory/src/file_loader.rs` to:
1. In managed directory mode (`load_all` default path), use XDG data dir instead of `~/{DIR_NAME}/` for user-level files
2. In dot-directory mode (`use_dot_directory_paths`), replace `~/.{subdirectory}` with `$XDG_DATA_HOME/{dir_name_without_dot}/{subdirectory}` for user-level, keep `{git_root}/{DIR_NAME}/{subdirectory}` for local
3. Update `get_directories()` to return XDG-resolved paths
4. Remove all `dirs::home_dir()` calls from this file — everything goes through XDG helpers or git root
5. Update `FileSource::User` comments

### Key insight
The dot-directory mode (`~/.prompts`, `~/.agents`) is what litters home dirs. These become `$XDG_DATA_HOME/sah/prompts/`, `$XDG_DATA_HOME/sah/agents/` — still accessed via VFS with same precedence, just XDG-compliant paths.

### Key files
- `swissarmyhammer-directory/src/file_loader.rs` — all changes here

## Acceptance Criteria
- [ ] No `dirs::home_dir()` calls remain in file_loader.rs
- [ ] XDG_DATA_HOME respected for user-level file loading
- [ ] Dot-directory mode resolves to XDG paths for user level
- [ ] get_directories() returns XDG-resolved paths
- [ ] All VFS tests pass

## Tests
- [ ] Update existing VFS tests for XDG paths
- [ ] Add test: XDG_DATA_HOME override changes user-level load path
- [ ] Add test: default fallback to ~/.local/share/ when XDG not set
- [ ] `cargo nextest run -p swissarmyhammer-directory`