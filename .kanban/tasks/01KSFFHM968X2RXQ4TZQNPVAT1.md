---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff80
title: 'mirdan AgentDef: add agent-agnostic preamble + settings paths'
---
## What

`mirdan::agents::AgentDef` (`crates/mirdan/src/agents.rs`) already encodes per-agent path knowledge for skills (`project_path`/`global_path`), MCP (`mcp_config`), plugins, and subagents — but it has **no concept of an agent's instructions/preamble file or its settings/permissions file**. That gap is why preamble + permissions install/doctor logic is hardcoded to Claude Code paths in the CLI instead of being data-driven and agent-agnostic. This card adds those two path families to the data model so later cards can resolve them generically.

Add four `#[serde(default)]` `Option<String>` fields to `AgentDef`, mirroring the existing `plugin_path`/`global_plugin_path`/`agent_path`/`global_agent_path` pattern:
- `instructions_path` — project-level preamble/instructions file (e.g. Claude Code: `CLAUDE.md`, relative to project root)
- `global_instructions_path` — global preamble file (Claude Code: `~/.claude/CLAUDE.md`)
- `settings_path` — project-level settings/permissions file (Claude Code: `.claude/settings.json`)
- `global_settings_path` — global settings file (Claude Code: `~/.claude/settings.json`)

Add accessor helpers next to the existing `agent_project_*`/`agent_global_*` fns (using `expand_tilde` for the global variants):
- `agent_project_instructions_file`, `agent_global_instructions_file`
- `agent_project_settings_file`, `agent_global_settings_file`

Populate **only the `claude-code` entry** in `crates/mirdan/src/agents_default.yaml` with the four new keys (all other 46 agents default to `None` → "not applicable" downstream; the data model supports adding more agents later without code changes).

Update the four `AgentDef` struct literals in the `agents.rs` test module (`test_get_detected_agents_fallback`, `test_agent_project_skill_dir`, and the two in `mock_config`) to include the new fields (set to `None`).

## Acceptance Criteria
- [x] `AgentDef` has the four new fields, all `#[serde(default)]`, documented with `///` doc comments.
- [x] Four accessor functions return the correct `Option<PathBuf>`, with `~` expanded for global variants.
- [x] `agents_default.yaml`'s `claude-code` entry sets `instructions_path: CLAUDE.md`, `global_instructions_path: "~/.claude/CLAUDE.md"`, `settings_path: .claude/settings.json`, `global_settings_path: "~/.claude/settings.json"`.
- [x] All other agent entries omit the new keys and still parse.
- [x] `cargo build -p mirdan` and existing `mirdan` tests compile/pass.

## Tests
- [x] Add `crates/mirdan/src/agents.rs` unit test: load the default config, find `claude-code`, assert all four new fields are `Some` with the expected values, and assert another agent (e.g. `aider`) has them all `None`.
- [x] Add accessor unit tests: build an `AgentDef` with `global_instructions_path: Some("~/.claude/CLAUDE.md")` and assert `agent_global_instructions_file` returns an absolute, tilde-expanded path; assert `agent_project_settings_file` returns the project-relative `PathBuf`.
- [x] `cargo test -p mirdan agents` runs green.

## Workflow
- Use `/tdd` — write the YAML-parsing and accessor tests first, watch them fail, then add fields + accessors + YAML. #init-doctor