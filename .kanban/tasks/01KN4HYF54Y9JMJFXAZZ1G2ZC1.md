---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffff9e80
title: Migrate AgentResolver to use VirtualFileSystem
---
AgentResolver in `swissarmyhammer-agents/src/agent_resolver.rs` hand-rolls the same file stacking pattern as SkillResolver instead of using the shared `VirtualFileSystem`.

**Current problems:**
1. Precedence is **builtin → local → user** (user wins over local) — should be **builtin → user → local** (local wins)
2. Duplicates VFS logic; already imports from `swissarmyhammer_directory` but doesn't use VFS

**What to do:**
- Replace hand-rolled `load_builtins()` / `load_from_local_paths()` / `load_from_user_paths()` with a `VirtualFileSystem` instance
- Use `vfs.use_dot_directory_paths()` for `~/.agents/` and `./.agents/` paths
- Register builtins via `vfs.add_builtin()`
- Precedence becomes correct automatically: builtin < user < local
- Keep `extra_paths` support via `vfs.add_search_path()`
- Keep `AgentSource` tracking — map from VFS `FileSource`
- Update tests to verify local > user > builtin shadowing

**Key files:**
- `swissarmyhammer-agents/src/agent_resolver.rs` (main target)
- `swissarmyhammer-directory/src/file_loader.rs` (VFS to use)
- `swissarmyhammer-prompts/src/prompt_resolver.rs` (reference implementation)

#refactor #vfs