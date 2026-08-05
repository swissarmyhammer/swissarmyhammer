---
assignees:
- claude-code
position_column: todo
position_ordinal: ff8a80
title: 'mirdan pre-existing review findings outside install/: store, search, mirdan-app'
---
29 review findings, split out of ^t1y1c37 (the install.rs split). All are pre-existing. These files entered review scope because the split's finding fixes touched them at single call sites (an `InstallMode` argument, one `copy_dir_recursive` consolidation). The findings cite production code the split never targeted. This mirrors how ^927239f was split out of ^t7ebyn8.

## Items

### crates/mirdan/src/store.rs
- Four store-dir functions `.expect("Could not find home directory")` on `dirs::home_dir()`. A missing home directory is an expected failure mode. Return `Result` and propagate. NOTE: this changes the signature of `skill_store_dir`/`agent_store_dir`/`tool_store_dir`/`validators_store_dir` and touches every caller across the workspace — plan the blast radius first.
- The repeated `"Could not find home directory"` literal becomes one named constant (moot if the `.expect()`s go away).
- `store_entry_still_referenced` is over the cognitive-complexity and nesting gates. Extract a `check_skill_dir_for_symlink` helper.

### crates/mirdan/src/search.rs
- Named constants for: short-query threshold `3`, default terminal size `(80, 24)` (two sites), result-limit bounds `2` and `20`, content margin `6` (the comment says "4-char left margin" — the comment and the value disagree; fix both), truncate-suffix width `2`.

### apps/mirdan-app/src/commands.rs
- `PackageInfo` and `SearchResult` need `Clone` in their derives.
- `SearchResult` fields `name`, `description`, `author`, `package_type` need doc comments.
- The discover call at line ~36 passes four adjacent bools — replace with an options struct or enums.
- `install_package`/`uninstall_package` are near-verbatim duplicates — extract one shared async helper (operation label + mirdan call + result formatter).
- Tauri command params take `String` where `&str` serves: `uninstall_package`, `update_package`, `get_registry_url`, `install_package`, `open_external`. Check what tauri::command requires before changing.
- Depth limit `5` (two sites) becomes `MAX_SKILL_STORE_DEPTH`.
- `find_in_store` is over the complexity (21) and nesting (5) gates — extract the file-matching logic.

### apps/mirdan-app/src/deeplink.rs
- `handle_url` takes `String` where `&str` serves.

## Warning on line numbers

Review line numbers track a diff pre-image. Grep for the symbol; do not trust the number.

## Acceptance

- Every item above fixed across its whole file, not only at cited lines.
- `cargo nextest run --workspace`, `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` clean. #refactor