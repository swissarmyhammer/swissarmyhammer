---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffa580
title: Migrate SkillResolver to use VirtualFileSystem
---
SkillResolver in `swissarmyhammer-skills/src/skill_resolver.rs` hand-rolls the builtin→local→user file stacking pattern instead of using the shared `VirtualFileSystem` from `swissarmyhammer-directory/src/file_loader.rs`.

**Current problems:**
1. Precedence is **builtin → local → user** (user wins over local) — should be **builtin → user → local** (local wins) to match VFS and all other resolvers
2. Duplicates logic already in VFS: directory walking, source tracking, HashMap insert-overwrites

**What to do:**
- Replace the hand-rolled `load_builtins()` / `load_from_local_paths()` / `load_from_user_paths()` with a `VirtualFileSystem` instance
- Use `vfs.use_dot_directory_paths()` for the `~/.skills/` and `./.skills/` paths (same as PromptResolver does)
- Register builtins via `vfs.add_builtin()`
- Precedence becomes correct automatically: builtin < user < local
- Keep `extra_paths` support (VFS has `add_search_path`)
- Keep `SkillSource` tracking — map from VFS `FileSource`
- Update tests to verify local > user > builtin shadowing

**Key files:**
- `swissarmyhammer-skills/src/skill_resolver.rs` (main target)
- `swissarmyhammer-directory/src/file_loader.rs` (VFS to use)
- `swissarmyhammer-prompts/src/prompt_resolver.rs` (reference implementation)

#refactor #vfs