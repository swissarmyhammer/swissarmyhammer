---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffff8480
title: Fix cargo fmt formatting violations in swissarmyhammer-cli and swissarmyhammer-tools
---
Two files have formatting diffs that fail `cargo fmt --check`:

**swissarmyhammer-cli/src/cli.rs** (around line 980, `test_tools_global_flag_with_enable`):
rustfmt wants to collapse the array literal into a single line:
```
let result =
    Cli::try_parse_from_args(["swissarmyhammer", "tools", "--global", "enable", "shell"]);
```
instead of the current multi-line `[...]` form.

**swissarmyhammer-tools/src/mcp/tool_config.rs** (around line 382):
rustfmt wants to collapse two `Arc::new(RwLock::new(...))` expressions to single lines:
```
let agent_lib = Arc::new(RwLock::new(swissarmyhammer_agents::AgentLibrary::new()));
let skill_lib = Arc::new(RwLock::new(swissarmyhammer_skills::SkillLibrary::new()));
```

Fix: run `cargo fmt -p swissarmyhammer-tools -p swissarmyhammer-cli` to auto-apply. #test-failure