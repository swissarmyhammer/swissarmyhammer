---
assignees:
- claude-code
position_column: doing
position_ordinal: '80'
title: Skill `profiles` metadata → install only profile-matched skills at tool init
---
Make builtin skills self-declare which init profile(s) they belong to (profile = tool); a tool's init deploys only the skills tagged for it. Today the kanban app deploys ALL 22 builtin skills from scratch on every board open (~19s, blocks the window). After this, kanban tool init deploys only the 6 `kanban`-profile skills, idempotently.

First pass already committed as `3b43777e5` (on `main`, NOT pushed). Second pass `e67021064`. This card now also covers the tool-Initializable restructure below; make a FOLLOW-UP commit on top.

## LOCKED — profile membership (profile = tool)
- **`kanban` profile** (deployed by kanban tool init): `kanban`, `plan`, `task`, `finish`, `implement`, `review`.
- **`code-context` profile** (belongs to code_context tool): `explore`, `code-context` — TAG them, but do NOT wire deployment (that lands in the later tool-surface card).
- All other builtins untagged (sah-only).

## LOCKED — behavior (tool-`Initializable` model)
- **A workspace is a SET OF TOOLS.** Every tool has an `Initializable` (its init). Ensuring a workspace = running each of its tools' `Initializable`s for the board scope (`InitScope::Project` rooted at the board dir). The kanban board workspace's tool set is currently just `[kanban]`.
- **Ensuring the kanban tool = running its init = deploying its profile skills.** There is NO generic "SAH workspace" step on the app's board-open path: no `ProjectStructure` / `StructureSetup`, no `.prompts/`, no `workflows/`. The workspace is exactly its tools; the `.sah/skills/` dir is created by the tool's own deploy step.
- The board-open step is named **`ensure_workspace_tools`** (renamed from `ensure_kanban_workspace`/`ensure_sah_workspace`) and iterates the workspace tool set, registering each tool's `Initializable`. It calls `swissarmyhammer_workspace_init::run_workspace_tools_init`, whose `WORKSPACE_TOOLS = ["kanban"]` table registers `SkillDeployment::for_profile(root, "kanban")` (no `ProjectStructure`).
- Deploy **BLOCKS before the kanban tool's MCP server starts**: run the tool inits, THEN `start_board_mcp_server`, in order, synchronously. NO fire-and-forget / `spawn_blocking`. (The `skill` tool resolves from `<board>/.sah/skills/`, so skills must exist before the server serves them.) Fast because it's 6 idempotent skills, not 22-from-scratch.
- **App deploy target UNCHANGED:** kanban-profile skills land in `<board>/.sah/skills/`. The app does NOT use the CLI's mirdan/agent-dir (`.claude/skills`) deploy.
- **kanban-CLI `kanban init` is a DIFFERENT consumer** (out of scope): its `KanbanTool` + `KanbanSkillDeployment` still deploy the 6 kanban-profile skills to detected-agent dirs via mirdan. Both are conceptually "the kanban tool's init" but target different stores; kept separate (no forced shared abstraction).
- `kanban deinit` removes the **full kanban profile** (decision **W1 = b**, symmetric with init) — keep current behavior.

## Implementation
1. `profiles: Vec<String>` proper YAML list — DONE in `3b43777e5` (skill.rs, skill_loader.rs `#[serde(default)]`).
2. Retag: `explore` + `code-context` on `profiles: [code-context]`; the other 6 on `[kanban]` — DONE in `e67021064`.
3. workspace-init: add `WORKSPACE_TOOLS` table + `run_workspace_tools_init` (registers `SkillDeployment::for_profile` per tool, NO `ProjectStructure`). Exported from lib.rs. `run_workspace_init_for_profile` kept for the `sah init` profile slice (still includes `ProjectStructure`); `ProjectStructure` still exported (sah CLI uses it).
4. kanban app `ensure_workspace_tools` (apps/kanban-app/src/state.rs): rename from `ensure_kanban_workspace`; drop the `KANBAN_PROFILE` const; call `run_workspace_tools_init` synchronously BEFORE `start_board_mcp_server`; keep idempotent skip.

## Review follow-ups (all folded in)
- [x] W1 — deinit removes full kanban profile: DECIDED = b (keep). No change.
- [x] W2 — deploy-vs-server race / dropped JoinHandle: RESOLVED by blocking-before-tool-start (no spawn_blocking; `ensure_workspace_tools` calls `deploy_workspace_tools` → `run_workspace_tools_init` synchronously before `start_board_mcp_server`).
- [x] W3 — `start_board_mcp_server` doc updated: the board's workspace tools are ensured synchronously by `ensure_workspace_tools` before the server starts (no race).
- [x] W4 — `apps/kanban-app/tests/workspace_init.rs` re-pointed at `run_workspace_tools_init`; asserts the 6 kanban skills deploy, explore/code-context/commit do not, and `.prompts/` is NOT created (tool-set model, no generic SAH workspace).
- [x] N1 — `components.rs` `write_skill` idempotency comment notes it compares only SKILL.md content and relies on the embedded `{{version}}` to catch resource-file changes.
- [x] N2 — `KNOWN_PROFILES` set + `debug_assert!` in the deploy loop so a mistagged builtin profile fails loudly.

## Acceptance criteria
- kanban board-open runs the workspace tool set (currently `[kanban]`) and deploys exactly the 6 kanban-profile skills to `<board>/.sah/skills/`; `explore`/`code-context`/untagged are NOT deployed; `.prompts/`/`workflows/` are NOT created by the app.
- Untagged skills still parse (default `[]`).
- Board open: skills present before the MCP server starts (no race); no ~19s stall; reopen with skills current = no re-render.
- Tests updated: filter excludes code-context + untagged; workspace_init.rs covers the tool path; existing tests pass.

## Out of scope (later cards)
code-context-profile DEPLOYMENT wiring; per-backend tool surface (Claude=kanban domain, llama=kanban+agent kit); `shell` Replace/deny-bash; code_context init gating; `/.sah`+model-config read-only-CWD bug.