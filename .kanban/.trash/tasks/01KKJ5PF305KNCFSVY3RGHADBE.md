---
depends_on:
- 01KKJ5NPDMAT6TFDYFNJ7Y4KBK
position_column: todo
position_ordinal: a2
title: Replace dirs::home_dir() in agent_resolver.rs load_from_user_paths()
---
## What\nBoth `SkillResolver` and `AgentResolver` directly call `dirs::home_dir()` to construct user-level paths like `~/.skills`, `~/.swissarmyhammer/skills`, `~/.agents`, `~/.swissarmyhammer/agents`. They should use the VFS or `ManagedDirectory::from_user_home()` so XDG changes propagate automatically.\n\nAffected files:\n- `swissarmyhammer-skills/src/skill_resolver.rs` (lines 144, 264)\n- `swissarmyhammer-agents/src/agent_resolver.rs` (line 108)\n\nApproach: Create a helper in `swissarmyhammer-directory` that returns the user config root (XDG-aware), then use that in the resolvers instead of `dirs::home_dir()`. Or refactor to use `VirtualFileSystem` with appropriate search paths.\n\n## Acceptance Criteria\n- [ ] `skill_resolver.rs` no longer calls `dirs::home_dir()` directly\n- [ ] `agent_resolver.rs` no longer calls `dirs::home_dir()` directly\n- [ ] Skills and agents still load from user-level directories\n- [ ] User-level paths now resolve via XDG\n\n## Tests\n- [ ] Existing skill/agent loading tests still pass\n- [ ] `cargo test -p swissarmyhammer-skills -p swissarmyhammer-agents`