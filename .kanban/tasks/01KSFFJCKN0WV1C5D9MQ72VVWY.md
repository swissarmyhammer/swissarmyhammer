---
assignees:
- claude-code
depends_on:
- 01KSFFHM968X2RXQ4TZQNPVAT1
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8280
title: 'mirdan: agent-agnostic install-status API (status module)'
---
## What

Give mirdan a single, agent-agnostic way to answer "is this sah-managed thing installed?" for any detected agent, for both project and user scope. This is the capability the doctor (and optionally `mirdan` itself) consumes instead of hand-coding Claude-specific path checks. "Smart not copy-paste" lives here: one data-driven detector keyed off `AgentDef` + scope, not N bespoke checks.

Create `crates/mirdan/src/status.rs` and export it from `crates/mirdan/src/lib.rs` (`pub mod status;`). Reuse `swissarmyhammer_common::lifecycle::InitScope` for scope (mirdan already depends on `swissarmyhammer-common`) — do NOT invent a new scope enum.

Types:
- `enum Component { Mcp, Skills, Agents, Preamble, Permissions }` (with a `fn label(&self) -> &str`).
- `enum ComponentState { Installed, Missing, NotApplicable }`.
- `struct ComponentStatus { agent_id: String, agent_name: String, component: Component, scope: InitScope, path: Option<PathBuf>, state: ComponentState, detail: String }`.

Functions:
- `pub fn component_path(agent: &AgentDef, component: Component, scope: InitScope) -> Option<PathBuf>` — resolves the on-disk location for that component+scope from `AgentDef` (map `User`→global accessors, `Project`/`Local`→project accessors): Mcp→`agent_*_mcp_config`, Skills→`agent_*_skill_dir`, Agents→`agent_*_agent_dir`, Preamble→`agent_*_instructions_file`, Permissions→`agent_*_settings_file`.
- `pub fn check_component(agent: &AgentDef, component: Component, scope: InitScope) -> ComponentStatus`.
- `pub fn check_agent(agent: &AgentDef, scope: InitScope) -> Vec<ComponentStatus>` — all components for one agent/scope.
- `pub fn check_all(config: &AgentsConfig, scopes: &[InitScope]) -> Vec<ComponentStatus>` — over `get_detected_agents` × scopes × components.
- `pub fn to_check(status: &ComponentStatus) -> swissarmyhammer_doctor::Check` — maps Installed→Ok, Missing→Warning (with a `sah init`/`sah init user` fix hint derived from scope), NotApplicable→Ok-but-skippable (caller decides; see card for doctor wiring).

Detection rules (NotApplicable when `component_path` is `None`):
- **Mcp**: read JSON at the mcp config path; Installed if `<servers_key>.sah` exists and its `command` is `sah` or ends with `/sah` (reuse the same predicate idea as `mcp_config`). Missing otherwise.
- **Skills**: Installed if the skill dir exists and is non-empty; Missing otherwise.
- **Agents**: Installed if the subagent dir exists and is non-empty; Missing otherwise.
- **Preamble**: Installed if the instructions file exists and its first non-empty line contains the preamble marker; Missing otherwise.
- **Permissions**: Installed if the settings JSON has `"Bash"` in `permissions.deny`; Missing otherwise.

Move the preamble marker constant to mirdan as the single source of truth: define `pub const PREAMBLE_MARKER: &str = "MANDATORY: load the thoughtful skill";` in `status.rs` (or a small `mirdan::preamble` module). The CLI's `CLAUDE_MD_PREAMBLE` in `apps/swissarmyhammer-cli/src/commands/install/components/mod.rs` must be re-pointed to re-export this constant (update that const to `pub use mirdan::status::PREAMBLE_MARKER as CLAUDE_MD_PREAMBLE;` or equivalent) so there is exactly one definition. Verify the existing doctor/component tests that reference `CLAUDE_MD_PREAMBLE` still compile.

## Acceptance Criteria
- [x] `mirdan::status` compiles and is exported; `Component`, `ComponentState`, `ComponentStatus`, and the five functions exist and are documented.
- [x] `check_all` returns one `ComponentStatus` per (detected agent, scope, component); for a synthetic config whose only detectable agent is `claude-code`, the result contains both Project and User rows for all five components.
- [x] `PREAMBLE_MARKER` is defined once in mirdan; the CLI re-exports it; no duplicate string literal of the marker remains in the CLI.
- [x] `cargo build -p mirdan -p swissarmyhammer-cli` is green.

## Tests
- [x] `crates/mirdan/src/status.rs` unit tests using a `tempfile::TempDir` and a hand-built `AgentDef` whose paths point inside the temp dir: assert Mcp/Skills/Agents/Preamble/Permissions each return `Installed` when the artifact is written and `Missing` when absent, and `NotApplicable` when the corresponding `AgentDef` path field is `None`.
- [x] Test `to_check` maps the three states to the expected `CheckStatus` and that Missing carries a non-empty `fix`.
- [x] `cargo test -p mirdan status` runs green.

## Workflow
- Use `/tdd` — write the per-component temp-dir tests first, then implement detection. #init-doctor

## Implementation Notes
- `component_path` for `Skills` always returns `Some` because `project_path`/`global_path` are required `AgentDef` fields; `NotApplicable` therefore applies to Mcp/Agents/Preamble/Permissions when their optional path field is `None`. Tests cover this.
- `to_check` check name is `"<Agent> · <scope> · <Component label>"`; Missing fix is `Run \`sah init\`` (Project/Local) or `Run \`sah init user\`` (User).
- Mcp detection probes both `mcpServers` and `servers` keys for the `sah` entry to cover agents that nest under a non-default key.