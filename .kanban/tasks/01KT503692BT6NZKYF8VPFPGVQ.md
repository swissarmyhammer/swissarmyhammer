---
assignees:
- claude-code
position_column: doing
position_ordinal: '80'
title: Skill `profiles` metadata → install only profile-matched skills at tool init
---
Make builtin skills self-declare which init profile(s) they belong to (profile = tool); a tool's init deploys only the skills tagged for it. Today the kanban app deploys ALL 22 builtin skills from scratch on every board open (~19s, blocks the window). After this, kanban tool init deploys only the 6 `kanban`-profile skills, idempotently.

First pass already committed as `3b43777e5` (on `main`, NOT pushed). This card now also covers the review follow-ups below; make a FOLLOW-UP commit on top.

## LOCKED — profile membership (profile = tool)
- **`kanban` profile** (deployed by kanban tool init): `kanban`, `plan`, `task`, `finish`, `implement`, `review`.
- **`code-context` profile** (belongs to code_context tool): `explore`, `code-context` — TAG them, but do NOT wire deployment (that lands in the later tool-surface card).
- All other builtins untagged (sah-only).

## LOCKED — behavior
- `ensure_sah_workspace` → **rename to `ensure_kanban_workspace`** (it inits a kanban workspace, not a SAH one).
- Deploy **BLOCKS before the kanban tool starts**: deploy the kanban-profile skills, THEN `start_board_mcp_server`, in order, synchronously. NO fire-and-forget / `spawn_blocking`. (The `skill` tool resolves from `.sah/skills/`, so skills must exist before the server serves them.) Fast because it's 6 idempotent skills, not 22-from-scratch.
- `kanban deinit` removes the **full kanban profile** (decision **W1 = b**, symmetric with init) — keep current behavior.

## Implementation
1. `profiles: Vec<String>` proper YAML list — DONE in `3b43777e5` (skill.rs, skill_loader.rs `#[serde(default)]`).
2. Retag: move `explore` + `code-context` from `profiles: [kanban]` → `profiles: [code-context]` (builtin/skills/{explore,code-context}/SKILL.md). Leave the other 6 on `[kanban]`.
3. kanban app `ensure_kanban_workspace` (apps/kanban-app/src/state.rs): rename; deploy kanban profile synchronously BEFORE `start_board_mcp_server`; remove the `spawn_blocking` backgrounding; keep idempotent skip.

## Review follow-ups (fold into the follow-up commit)
- [x] W1 — deinit removes full kanban profile: DECIDED = b (keep). No change.
- [x] W2 — deploy-vs-server race / dropped JoinHandle: RESOLVED by blocking-before-tool-start (removed spawn_blocking; `ensure_kanban_workspace` now calls `deploy_kanban_workspace` synchronously before `start_board_mcp_server`).
- [x] W3 — `start_board_mcp_server` doc updated to state the `.sah` workspace is created synchronously by `ensure_kanban_workspace` before the server starts (no race).
- [x] W4 — `apps/kanban-app/tests/workspace_init.rs` re-pointed at `run_workspace_init_for_profile(.., "kanban", ..)`; asserts the 6 kanban skills are deployed and explore/code-context/commit are not.
- [x] N1 — `components.rs` `write_skill` idempotency comment now notes it compares only SKILL.md content and relies on the embedded `{{version}}` to catch resource-file changes.
- [x] N2 — added `KNOWN_PROFILES` set + `debug_assert!` in the deploy loop so a mistagged builtin profile (typo/case-mismatch) fails loudly instead of silently dropping.

## Acceptance criteria
- kanban init/board-open deploys exactly the 6 kanban-profile skills; `explore`/`code-context`/untagged are NOT deployed by kanban.
- Untagged skills still parse (default `[]`).
- Board open: skills are present before the MCP server starts (no race); window appears without the ~19s stall; reopen with skills current = no re-render.
- Tests updated: filter excludes code-context + untagged; workspace_init.rs covers the profile path; existing tests pass.

## Out of scope (later cards)
code-context-profile DEPLOYMENT wiring; per-backend tool surface (Claude=kanban domain, llama=kanban+agent kit); `shell` Replace/deny-bash; code_context init gating; `/.sah`+model-config read-only-CWD bug.