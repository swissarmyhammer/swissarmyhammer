---
assignees:
- assistant
depends_on:
- 01KKM7RX9ZTK4VKMB6P9NPWHWW
- 01KKM7SG4QPRPNWJSZ8M1BB5H1
position_column: done
position_ordinal: ffffb680
title: Eliminate hardcoded home_dir paths in agents, mirdan/AVP, and prompts
---
## What
Three subsystems bypass the VFS and call `dirs::home_dir()` directly to construct dot-dir paths. Route them through `ManagedDirectory` XDG constructors instead.

### 1. Agents (`swissarmyhammer-agents/src/agent_resolver.rs`)
- Lines 99-113: `cwd.join(\".agents\")`, `home.join(\".agents\")`, `cwd.join(\".swissarmyhammer\").join(\"agents\")`, `home.join(\".swissarmyhammer\").join(\"agents\")`
- Should use VFS or ManagedDirectory for both local (git root) and user (XDG) paths
- Agent source resolution: builtin → user (`$XDG_DATA_HOME/sah/agents/`) → local (`{git_root}/.agents/` or `{git_root}/.sah/agents/`)

### 2. Mirdan/AVP (`mirdan/src/new.rs`)
- Line 120-122: `dirs::home_dir().join(\".avp\").join(\"validators\")`
- Line 242: similar pattern
- `mirdan/src/cli.rs`: help text references `.avp/validators/` and `~/.avp/validators/`
- `mirdan/src/git_source.rs`: Lines 308, 451-453: hardcoded `.avp` references in git source scanning
- Should use `ManagedDirectory::<AvpConfig>::xdg_data()` for global, `from_git_root()` for local

### 3. Prompts (`swissarmyhammer-prompts/src/prompt_resolver.rs`)
- Uses VFS dot-directory mode (`~/.prompts`) which will be fixed by card 2
- But has hardcoded comments and test paths referencing `~/.prompts`

### Key files
- `swissarmyhammer-agents/src/agent_resolver.rs`
- `mirdan/src/new.rs`, `mirdan/src/cli.rs`, `mirdan/src/git_source.rs`
- `swissarmyhammer-prompts/src/prompt_resolver.rs`

## Acceptance Criteria
- [ ] No `dirs::home_dir().join(\".agents\")` or `.join(\".avp\")` anywhere
- [ ] All user-level paths resolve via ManagedDirectory XDG constructors
- [ ] Project-local paths use `from_git_root()` or VFS
- [ ] CLI help text updated to show XDG paths

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-agents`
- [ ] `cargo nextest run -p mirdan`
- [ ] `cargo nextest run -p swissarmyhammer-prompts`