---
depends_on:
- 01KKC8A9RVGEJP5XBVJH6H3G5V
position_column: done
position_ordinal: ffffffffffff9580
title: 'STATUSLINE-2: DirectoryConfig + config stacking'
---
## What
Add `StatuslineDirectoryConfig` to `swissarmyhammer-directory` and implement YAML config loading with the same stacked pattern as the shell tool. Create the builtin default config.

Key files:
- `swissarmyhammer-directory/src/config.rs` — add `StatuslineDirectoryConfig` impl
- `swissarmyhammer-directory/src/lib.rs` — re-export it
- `swissarmyhammer-statusline/src/config.rs` (new) — config types + `load_statusline_config()` using VFS
- `builtin/statusline/config.yaml` (new) — default format and module configs

Follow `swissarmyhammer-shell/src/config.rs` pattern exactly:
- `VirtualFileSystem<StatuslineDirectoryConfig>::new(\"statusline\")`
- `vfs.add_builtin(\"config\", BUILTIN_CONFIG_YAML)`
- `vfs.use_dot_directory_paths()` — discovers `~/.swissarmyhammer/statusline/` and `.swissarmyhammer/statusline/`
- `merge_config_stack()` iterates layers, later overrides earlier

Merge semantics:
- `format` string: last layer wins entirely
- Module sections (flattened HashMap): each key replaced wholesale by later layer
- Missing layers silently skipped

## Acceptance Criteria
- [ ] `StatuslineDirectoryConfig` added to swissarmyhammer-directory with DIR_NAME `.statusline`
- [ ] `builtin/statusline/config.yaml` exists with sensible defaults
- [ ] `load_statusline_config()` returns merged config from builtin + user + project
- [ ] Config merging: later layer format replaces earlier; module sections replaced per-key

## Tests
- [ ] Unit test: parse builtin config.yaml successfully
- [ ] Unit test: merge two configs — format from later wins
- [ ] Unit test: merge two configs — module section from later replaces earlier
- [ ] Unit test: load with no overlays returns builtin defaults
- [ ] Unit test: load with overlay dir merges correctly
- [ ] `cargo test -p swissarmyhammer-statusline`