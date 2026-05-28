---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffad80
title: 'Doctor: extend agents_default.yaml path coverage for the 4 doctored agents'
---
## What

`mirdan::status::check_all` produces one `ComponentStatus` per (agent, scope, component), but in `crates/mirdan/src/agents_default.yaml` only Claude Code has the full set of path fields (`mcp_config`, `agent_path` / `global_agent_path`, `instructions_path` / `global_instructions_path`, `settings_path` / `global_settings_path`). Zed AI (`zed-ai`), GitHub Copilot (`copilot`), and Codex (`codex`) only define `project_path` / `global_path` for skills, so `mirdan::status::component_path` returns `None` for everything else and the doctor reports them as `NotApplicable` (filtered out).

The four agents we doctor are Claude Code, Zed AI, GitHub Copilot, Codex. Fill in their real paths and also mark them as doctored via the YAML `doctor: true` field that card **01KSMXJBVVH06V6EDHYCFCRBHS** introduces (so this card and that one converge on the same four YAML entries). Their real-world paths:

- **Claude Code** (`claude-code`): already complete; just add `doctor: true`.
- **Zed AI** (`zed-ai`): MCP servers live in `~/.config/zed/settings.json` under the `"context_servers"` key; projects override via `.zed/settings.json`. Zed has no preamble file, no subagents directory; "skills" don't map cleanly either, but `~/.config/zed/prompts/` is the closest analog.
- **GitHub Copilot** (`copilot`): user instructions in `~/.config/github-copilot/intellij/global-copilot-instructions.md`, project instructions in `.github/copilot-instructions.md`. Copilot's MCP support is evolving; keep what mirdan can probe today and leave `mcp_config` unset if there is no stable convention.
- **Codex** (OpenAI Codex CLI, `codex`): config at `~/.codex/config.toml` (TOML, not JSON — handled by **01KSMXHQ7M8N38VM0ZD5P415TJ**), instructions at `~/.codex/AGENTS.md` (user) and `AGENTS.md` (project). No subagents directory.

Fill in only the fields that genuinely exist for each agent — leave the rest unset so they resolve to `NotApplicable` and the doctor skips them (rather than fabricating phantom paths).

Files:
- `crates/mirdan/src/agents_default.yaml` — add `doctor: true` and the relevant path fields for `claude-code`, `zed-ai`, `copilot`, `codex`. Leave every other entry as-is.

## Acceptance Criteria
- [ ] `claude-code`, `zed-ai`, `copilot`, `codex` each have `doctor: true` set in `agents_default.yaml`. No other agent does.
- [ ] `zed-ai` entry defines `mcp_config` (with `project_path: .zed/settings.json`, `global_path: ~/.config/zed/settings.json`, `servers_key: context_servers`); no `instructions_path` or `agent_path`.
- [ ] `copilot` entry defines `instructions_path: .github/copilot-instructions.md` and `global_instructions_path: ~/.config/github-copilot/intellij/global-copilot-instructions.md`; no `agent_path`.
- [ ] `codex` entry defines `mcp_config` (with `project_path: .codex/config.toml`, `global_path: ~/.codex/config.toml`, `servers_key: mcp_servers`) and `instructions_path: AGENTS.md` / `global_instructions_path: ~/.codex/AGENTS.md`; no `agent_path`.
- [ ] `cargo run --bin sah doctor` shows full per-component rows for each detected agent among the 4 (verified by adding a Codex layout in a test).
- [ ] The existing default `serde_yaml::from_str` test in `crates/mirdan/src/agents.rs` (whatever validates `agents_default.yaml` parses) still passes.

## Tests
- [ ] Add a unit test in `crates/mirdan/src/agents.rs` (or a new test in `crates/mirdan/src/status.rs`) that loads the default config, finds each of `claude-code`, `zed-ai`, `copilot`, `codex`, and asserts the new path fields are populated as above. Test command: `cargo test -p mirdan agents_default_doctored_paths`.
- [ ] Update or add a test in `crates/mirdan/src/status.rs` that, given a synthetic `AgentsConfig` containing the augmented `codex` entry plus a tempdir-based fake `~/.codex/config.toml`, asserts `check_all` produces `Installed`/`Missing` rows for MCP and Preamble at both scopes (not `NotApplicable`). Test command: `cargo test -p mirdan codex_full_stack`.

## Workflow
- Use `/tdd` — add the failing tests first, then edit the YAML to make them pass.

## Note on `doctor: true`
The `doctor: bool` field itself is added to the `AgentDef` schema by **01KSMXJBVVH06V6EDHYCFCRBHS**. Either land that card first and set the field here, or land both together — coordinate with the implementer so the YAML edits and the schema land in the same passing build.