---
assignees:
- assistant
position_column: done
position_ordinal: fffffff280
title: Make ralph a proper subsystem with DirectoryConfig, ManagedDirectory, and .ralph/
---
## What
Ralph currently stores session files by manually joining `.sah/ralph/`. It should be a proper subsystem like `.avp`, `.shell`, `.code-context` — with its own `DirectoryConfig` impl, `ManagedDirectory` integration, and `.ralph/` project-local directory.

### 1. Add `RalphConfig` to `swissarmyhammer-directory/src/config.rs`
```rust
pub struct RalphConfig;
impl DirectoryConfig for RalphConfig {
    const DIR_NAME: &'static str = ".ralph";
    const XDG_NAME: &'static str = "ralph";
    const GITIGNORE_CONTENT: &'static str = "# Ralph session state\n# All files are ephemeral per-session instructions\n*\n!.gitignore\n";
}
```
The `*` + `!.gitignore` pattern ignores everything except the gitignore itself — ralph is fully ephemeral.

### 2. Re-export from `swissarmyhammer-directory/src/lib.rs`
Add `RalphConfig` to the config re-exports.

### 3. Update `swissarmyhammer-tools/src/mcp/tools/ralph/state.rs`
- Replace manual `base_dir.join(".sah").join("ralph")` with `ManagedDirectory::<RalphConfig>::from_custom_root(base_dir)`
- Use `dir.root()` to get the path
- Import from `swissarmyhammer_directory`

### 4. Update `swissarmyhammer-tools/src/mcp/tools/ralph/execute/mod.rs`
- Doc comments: `.sah/ralph/` → `.ralph/`
- Description strings: same
- Init logic: use ManagedDirectory instead of manual mkdir

### 5. Update `swissarmyhammer-tools/src/mcp/tools/ralph/mod.rs`
- Doc comments

### 6. No root .gitignore entry needed
Ralph's own `GITIGNORE_CONTENT` handles ignoring everything inside `.ralph/`. The root `.gitignore` does NOT need a `.ralph/` entry.

## Acceptance Criteria
- [ ] `RalphConfig` exists with DIR_NAME=`.ralph`, XDG_NAME=`ralph`
- [ ] `ralph_dir()` uses `ManagedDirectory::<RalphConfig>` not manual path joins
- [ ] `.ralph/.gitignore` auto-created by ManagedDirectory with `*` pattern
- [ ] All docs updated
- [ ] Tests pass: `cargo nextest run -p swissarmyhammer-directory -p swissarmyhammer-tools`