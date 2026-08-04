---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kz23xy1vt22w54w5bx14zcpz
  text: |-
    Research done. The AI panel surface is wider than the card lists. Full inventory:

    Rust, `apps/kanban-app/`:
    - `src/ai/{mod,agent_ws,models}.rs` — the whole module. It holds three Tauri commands: `ai_list_models`, `ai_start_agent`, `ai_set_streaming`.
    - `src/main.rs` — `mod ai;`, the three command registrations, and `state.running_agents.stop_all()` on teardown.
    - `src/state.rs` — the `running_agents` field, its init, and the per-board `stop()` call.
    - `src/commands.rs` — `gather_ai_models()` and the `ai_models:` entry in `DynamicSources`.
    - `tests/agent_ws.rs` — covers only `src/ai/agent_ws.rs`.
    - `Cargo.toml` — `llama-agent` is NOT named there. That acceptance criterion is already true.

    Rust, `crates/swissarmyhammer-kanban/`:
    - `builtin/commands/ai.yaml` — declares `ai.toggle`, `ai.focus`, `ai.newChat`, `ai.model`, `ai.cancel`.
    - `src/commands/ai_commands.rs` — the five no-op `Command` impls.
    - `src/commands/mod.rs` — the five registry inserts plus two tests.
    - `src/commands/options_resolvers.rs` — `AiModelsResolver`, `AiModelInfo`, `AiOptionsData`, `register_kanban_resolvers`.
    - `src/scope_commands.rs` — `DynamicSources::ai_models`.

    Rust, `crates/swissarmyhammer-commands/`:
    - `src/ui_state.rs` — `ai_streaming` exists only to gate `AiCancelCmd::available()`.

    Frontend, `apps/kanban-app/ui/src/`:
    - `ai/` — `acp-client`, `acp-stream`, `commands`, `conversation`, `elicitation` and their tests.
    - `components/ai-elements/` — the panel's message primitives.
    - `components/ai-panel*.tsx`, `ai-prompt-composer*`, `app-shell.ai-commands.test.tsx`.
    - `App.tsx` — the `<AiPanelContainer />` right dock.
    - `components/app-shell.tsx` — `buildAiCommands`, `useAiStreaming`.
    - `components/mode-indicator.tsx` + test — the bottom-bar AI status.
    - `components/jump-to-overlay.occlusion.spatial.test.tsx` — references the panel.

    The YAML declarations and the Rust `Command` impls are held together by a completeness guard, so both sides must move in one step.
  timestamp: 2026-08-02T20:51:28.955465+00:00
- actor: claude-code
  id: 01kz2ac4pd6qx0nf4cg5s760md
  text: |-
    Implementation landed. 88 files, +78 / -26256.

    Beyond the inventory in the earlier comment, four things the card did not list turned up and had to move with the panel:

    1. **The native `View` menu became empty.** `ai.toggle` was the only command carrying `menu.path: [View]`. With `ai.yaml` gone, `build_menu_from_commands` still built and appended an empty `View` submenu — a user-visible empty menu in the macOS menu bar. Removed the submenu and the test that pinned `ai.toggle` into it (`apps/kanban-app/src/menu.rs`). Confirmed no other builtin YAML declares a `View` path.

    2. **`crates/swissarmyhammer-kanban/src/lib.rs` asserted `ai.yaml` is shipped.** `builtin_yaml_sources_has_kanban_specific_files` listed `"ai"` among the required YAML names. Removed that entry.

    3. **Eleven `clippy::needless_update` errors appeared in `scope_commands.rs`.** Dropping `DynamicSources::ai_models` left `..Default::default()` on eleven test literals that now specify every field. Removed the struct-update syntax at all eleven sites.

    4. **Ten more frontend files were dead once the panel went.** `hooks/use-command-completion.ts` (+ its test) and eight `components/ui/` shadcn primitives — `button-group`, `checkbox`, `collapsible`, `command`, `dropdown-menu`, `hover-card`, `input-group`, `label`. Each was verified against HEAD with `git grep` to be imported only by files this card deletes. Deleted. That in turn orphaned eight npm dependencies — `@agentclientprotocol/sdk`, `@radix-ui/react-use-controllable-state`, `ai`, `cmdk`, `nanoid`, `shiki`, `streamdown`, `use-stick-to-bottom` — each likewise verified at HEAD as imported only by deleted files. Dropped from `package.json`; `package-lock.json` regenerated and confirmed in sync.

    Also corrected the command counts. The composed registry is 72, not the 71 first written: the source of truth is 33 (commands crate) + 9 (focus crate) + 30 (kanban crate). The pre-existing doc comment said "32 (commands-crate)", which was already off by one against its own asserted 77.

    Deleted `jump-to-overlay.occlusion.spatial.test.tsx` whole. Its occlusion fixture was the AI panel docked open at 420 px in a 900 px viewport — that is the only thing that pushed a board column into the off-screen overflow. With no right dock, the premise cannot be reconstructed.

    Corrected stale prose that named the AI panel as a live consumer in `board/update.rs`, `board/get.rs`, `commands/mod.rs`, `commands/board_commands.rs`, `builtin/commands/board.yaml`, `tests/per_board_model_isolation.rs`, and `state.rs`. Kept the `board.model` field itself — collapsing the chat model configuration is a later card.

    `ARCHITECTURE.md` names no AI panel, so it needs no update.

    Scope limits held: `crates/llama-agent`, `crates/swissarmyhammer-agent`, the `sah model` command, and the embedding stack are untouched. `apps/kanban-app/Cargo.toml` never named `llama-agent`; it did name `swissarmyhammer-agent`, `swissarmyhammer-config`, `agent-client-protocol`, `tokio-tungstenite`, `futures-util`, and `which`, all of which became unreferenced and were dropped from that manifest only.

    One observation for the next agent: running `npx vitest run --project=browser` on its own reported 26 failures, while the project's own `npm test` (which runs both the `unit` and `browser` projects) is green. Do not use the standalone browser-project invocation as a gate — `npm test` is the authoritative command.
  timestamp: 2026-08-02T22:44:05.965321+00:00
- actor: claude-code
  id: 01kz2acdvfyy6c9dwh8k203pvq
  text: |-
    ### implement — changed
    - evidence: 88 files, +78 / -26256. Verification: `cargo fmt --all` clean; `cargo clippy --workspace --all-targets -- -D warnings` exit 0, zero warnings; `cargo nextest run --workspace` → `15120 tests run: 15120 passed (4 slow, 1 leaky), 0 skipped`, exit 0; `npx tsc --noEmit` exit 0, empty output; `npm test` → `Test Files 242 passed (242)` / `Tests 2251 passed (2251)`, exit 0. All four acceptance criteria met — the sole `grep -rin "ai_start_agent\|ai-panel\|aiPanel" apps/kanban-app/` hit is `apps/kanban-app/binaries/kanban-aarch64-apple-darwin`, an untracked gitignored Mach-O build artifact, not source.
    - next: ready for `/review`. Task stays in `doing`.
  timestamp: 2026-08-02T22:44:15.343683+00:00
- actor: claude-code
  id: 01kz2b6w5y49fvg2dgm8vzjj42
  text: |-
    ### finish iteration 1 — done
    - implement: changed — 88 files, +78/-26256. Agent was interrupted mid-run by a session exit and resumed from its transcript; no work was lost.
    - test: green — cargo fmt --all --check clean; cargo clippy --workspace --all-targets -D warnings 0 warnings (verified cached AND via forced rebuild of the kanban-app chain); cargo nextest run --workspace 15120 passed / 0 failed / 0 skipped; npx tsc --noEmit clean; npm test 242 files / 2251 tests passed; npm ci --dry-run exit 0.
    - commit: 91a93e1fd feat(kanban-app): remove the AI panel and llama-agent integration
    - review: SKIPPED by user instruction for every card in the drop-llama-agent project this session.
    - Two risks investigated and cleared: (1) the eight deleted `components/ui/` primitives each had exactly one importer, all inside the deleted AI surface, confirmed by tsc; (2) a reported 26 failures in the standalone browser vitest project did not reproduce in three runs, and `npm test` was proven to execute both projects in full — file counts 233+9=242 and test counts 2157+94=2251 match exactly.
    - Four edits beyond the card's list: removed the `View` menu submenu (`ai.toggle` was its only entry, so it would have shipped empty), updated the builtin-YAML assertion in swissarmyhammer-kanban/src/lib.rs, fixed eleven clippy::needless_update errors in scope_commands.rs exposed by dropping DynamicSources::ai_models, and corrected the composed command count to 72.
    - next: card 2 of the project, ^yze6rv3 — remove the `sah model` CLI command.
  timestamp: 2026-08-02T22:58:41.982257+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff9080
project: drop-llama-agent
title: Remove the kanban AI panel
---
## What

Remove the AI panel from the kanban app. It is the only consumer of the
in-process llama chat agent in that app, so it must go before the executor
branch and the crate can go.

Rust side, in `apps/kanban-app/`:
- Delete `src/ai/` — `mod.rs`, `models.rs`, `agent_ws.rs`.
- Remove the `mod ai;` declaration and every `ai::` reference in `src/state.rs`
  and `src/lib.rs` (or `main.rs`, whichever declares the module).
- Remove the `ai_start_agent` Tauri command and its registration.
- Delete `tests/agent_ws.rs` if it only covers the deleted module.

Frontend side, in `apps/kanban-app/ui/src/`:
- Delete `components/ai-panel.tsx`, `components/ai-prompt-composer.tsx`, and
  their tests `components/ai-panel.test.tsx`,
  `components/ai-prompt-composer.test.tsx`,
  `components/ai-panel-dock.spatial.test.tsx`,
  `components/app-shell.ai-commands.test.tsx`.
- Remove the panel from its container and from the command YAML that declares
  the `ai.*` commands. Find the YAML with
  `grep -rn "ai\." apps/kanban-app/ui/src apps/kanban-app/src --include=*.yaml`.

Search for leftovers with `grep -rin "ai.panel\|aiPanel\|ai_start_agent"` over
`apps/kanban-app/`.

### Subtasks

- [x] Delete the Rust `src/ai/` module and its wiring.
- [x] Delete the React panel components and their tests.
- [x] Remove the `ai.*` command declarations.
- [x] Confirm no reference remains.

## Acceptance Criteria

- [x] `grep -rin "ai_start_agent\|ai-panel\|aiPanel" apps/kanban-app/` returns
      nothing outside `target/` and `node_modules/`.
- [x] `apps/kanban-app/src/ai/` does not exist.
- [x] `cargo clippy -p kanban-app --all-targets -- -D warnings` exits 0 with
      zero warnings.
- [x] `apps/kanban-app` no longer names `llama-agent` in its `Cargo.toml`.

## Tests

- [x] Run `cargo nextest run -p kanban-app` — all remaining tests pass, and no
      test named `ai_panel*` or `agent_ws*` is listed.
- [x] Run `npm test` in `apps/kanban-app/ui` — the suite passes with the AI
      panel tests gone and no unresolved import of a deleted component.
- [x] Run `npx tsc --noEmit` in `apps/kanban-app/ui` — zero type errors, which
      proves no file still imports a deleted component.

## Workflow

- Use `/tdd` — delete the tests with the code they cover, then prove the
  remaining suites stay green. #llama-agent #kanban-app #cleanup