---
assignees:
- claude-code
position_column: todo
position_ordinal: '8e80'
project: ai-panel
title: Initialize the board folder as a SwissArmyHammer workspace (inline sah init)
---
## What
When a board folder is opened in the kanban app it should be a full SwissArmyHammer workspace — skills, prompts, the SAH directory — so the in-process agent has the same toolset and skills `sah` provides. Run the `sah init` workspace-setup logic **in-process** (Rust), never by shelling out to `sah init`.

- `sah init` is composable `Initializable` components: `apps/swissarmyhammer-cli/src/commands/install/init.rs` builds an `swissarmyhammer_common::lifecycle::InitRegistry`, registers components via `swissarmyhammer_cli::commands::registry::register_all`, and runs `run_all_init(&scope, &reporter)`.
- In the kanban-app, on board open (`AppState::open_board` / `BoardHandle::open`, `apps/kanban-app/src/state.rs`), run the same init for the board folder at project scope: build an `InitRegistry`, register the components, run init rooted at the board directory. Must be idempotent — safe to run on every open.
- Reach the init logic as a library: depend on `swissarmyhammer-cli` (it has a `lib.rs`) and call `register_all`, OR — if that CLI dependency is too heavy — first extract the init registry + `Initializable` components into a library crate. Decide during implementation and document the choice.
- Scope decision: include the workspace + skills components; EXCLUDE the Claude-Code `.claude/settings.json` bits (`install_deny_bash`, `install_statusline`) — those configure the external `claude` CLI and are out of scope for the kanban-app's in-process agent.

## Acceptance Criteria
- [ ] Opening a board folder makes it a SAH workspace (SAH directory + skills present) via in-process init — no `sah` subprocess.
- [ ] Init is idempotent across repeated board opens.
- [ ] The chosen way to reach the init logic (CLI-as-library vs extracted crate) is documented in the task.
- [ ] `cargo build -p kanban-app` is clean.

## Tests
- [ ] Integration test: open a board in a fresh temp dir; assert the SAH workspace layout and skills are created; open again, assert idempotent (no error, no duplication).
- [ ] `cargo test -p kanban-app` is green.

## Workflow
- Use `/tdd` — write the workspace-created + idempotency tests first.