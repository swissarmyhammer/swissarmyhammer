---
assignees:
- claude-code
depends_on:
- 01KT503692BT6NZKYF8VPFPGVQ
position_column: todo
position_ordinal: '8980'
title: Agent `profiles` metadata → install only profile-matched agents at tool init
---
Mirror the skill `profiles` mechanism for builtin AGENTS. A tool's init should deploy the profile-matched agents (to the board's `.sah/agents/`) the same way it deploys profile-matched skills. Depends on / extends the skill-profiles card `01KT503692BT6NZKYF8VPFPGVQ` (reuses the same `profiles` concept and the workspace-tools / kanban-tool `Initializable` deploy path).

## Why
Agents mirror skills almost exactly: `Agent` has source `builtin/agents/<name>/AGENT.md`, `AgentFrontmatter` already parses list fields (`skills: Vec<String>`), `AgentLibrary::load_defaults()` loads builtins, and agents deploy to disk via `mirdan::install::deploy_agent_to_agents` → `.sah/agents/` (Project scope). So the kanban tool init should lay down its profile's agents, not all of them.

## Proposed membership (CONFIRM)
`kanban` profile agents = the subagents the kanban-profile skills actually fork to: **`implementer`** (implement skill, `agent: implementer`), **`reviewer`** (review skill, `agent: reviewer`), **`tester`** (finish's implement→test→review loop). NOT in kanban profile: `planner`/`plan` (plan skill doesn't fork), `committer` (commit skill is untagged), `explore` (now code-context profile), `default`/`general-purpose` (base/fallback — decide if the kanban agent needs them).

## Implementation (mirror the skill card)
1. `profiles` as a proper YAML list on agents:
   - crates/swissarmyhammer-agents/src/agent.rs — add `pub profiles: Vec<String>` to the `Agent` struct.
   - crates/swissarmyhammer-agents/src/agent_loader.rs — add `#[serde(default)] profiles: Vec<String>` to `AgentFrontmatter`; set it in `parse_agent_md_with_path` (untagged → `[]`). Note `skills: Vec<String>` already proves YAML-list parsing works here.
2. Tag the kanban-profile agents' source frontmatter `builtin/agents/<name>/AGENT.md` (NEVER a generated dir) with `profiles:\n  - kanban`: implementer, reviewer, tester (pending CONFIRM).
3. Extend the kanban tool's init (the `Initializable` run by `run_workspace_tools_init` from card 01KT5036…) to ALSO deploy profile-matched agents to `<board>/.sah/agents/`, idempotently (skip when on-disk AGENT.md already current), synchronously before `start_board_mcp_server`. Reuse the profile-filter helper + `KNOWN_PROFILES` validation from the skill card.
4. Keep the CLI's existing `kanban init` agent behavior (mirdan/agent-dirs) consistent with however the skill card left the CLI — out of scope to change unless trivial.

## Acceptance criteria
- A builtin agent tagged `profiles: [kanban]` is deployed by kanban tool init; untagged agents are not.
- Agents with no `profiles` key still parse (default `[]`) — existing agents unaffected.
- Board open deploys exactly the kanban-profile agents to `.sah/agents/` before the MCP server starts; reopen with agents current = no re-render.
- Tests: agent_loader parses a `profiles:` YAML list; profile filter selects the right subset (excludes untagged); existing agent tests pass.

## Out of scope
Per-backend agent surface, the broader tool-surface card, and anything beyond agent profile tagging + profile-filtered deploy at tool init.