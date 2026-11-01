---
assignees:
- claude-code
position_column: todo
position_ordinal: f980
title: 'tool_config.rs / health_registry.rs: fix latent findings found during ^0dystg9 self-check review'
---
## What

While fixing the 7 named review findings on ^0dystg9, a self-check review run (`review file` on both touched files) surfaced 7 additional findings against code that already existed before ^0dystg9's edits. ^0dystg9 only extracted constants and added derives; it did not introduce these patterns. Filed here so they get proper attention as their own task, per the "new work discovered goes on a new card" rule.

## Findings

- [ ] `crates/swissarmyhammer-tools/src/health_registry.rs` (in `validate_frontmatter_file`) — Error message "Failed to read file: {}" begins with a capital letter. Error-handling rule requires lowercase Display messages with no trailing punctuation. Change to "failed to read file: {}".
- [ ] `crates/swissarmyhammer-tools/src/health_registry.rs` (in `collect_yaml_errors_from_dir`) — The markdown extension check `e.path().extension().and_then(|s| s.to_str()) == Some("md")` is case-sensitive and misses `.MD` / `.Md` files. Use case-insensitive comparison: `.map(|s| s.to_ascii_lowercase()).as_deref() == Some("md")`. Add a regression test with a `.MD` file.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tool_config.rs` — `project_config_path()` and `global_config_path()` are near-verbatim copies, both doing `.join(SAH_CONFIG_DIR).join(TOOLS_CONFIG_FILENAME)` on a different base. Extract a parameterized helper, e.g. `resolve_config_path(base: impl Fn() -> Option<PathBuf>) -> Option<PathBuf>`, and call it from both.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tool_config.rs` — `ToolConfigWatcher::new()` computes `global_mtime`/`project_mtime` with the identical pattern `<path>.as_ref().and_then(|p| file_mtime(p))` repeated twice, and the same pattern repeats again in `check_and_reload()` for `new_global_mtime`/`new_project_mtime` (4 occurrences total across the two methods). Extract a shared helper/closure, e.g. `let get_mtime = |path: &Option<PathBuf>| path.as_ref().and_then(|p| file_mtime(p));`, and use it at all 4 call sites.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tool_config.rs` — In `check_and_reload()`, `self.project_mtime = new_project_mtime;` duplicates `self.global_mtime = new_global_mtime;` with only field names differing. Unify via a loop or paired-update helper.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tool_config.rs` (test `test_watcher_detects_file_change`) — Hardcoded `50` ms sleep configures test behavior (ensuring the mtime changes) and should be a named constant, e.g. `const TEST_MTIME_CHANGE_DELAY_MS: u64 = 50;`.

## Notes

None of these are regressions from ^0dystg9 — they predate it. Confirmed via `cargo nextest run -p swissarmyhammer-tools health_registry tool_config` (22/22 green) and `cargo clippy -p swissarmyhammer-tools --lib -- -D warnings` (clean) after ^0dystg9's fixes, so this card is purely additional cleanup, not a regression fix. #tech-debt #swissarmyhammer-tools