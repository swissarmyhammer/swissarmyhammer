---
assignees:
- claude-code
depends_on:
- 01KRRN5X6MWQ157CC9PN2QZHWT
position_column: todo
position_ordinal: 8a80
project: ai-panel
title: AI panel command scope and keybindings
---
## What
Make the AI panel a first-class citizen of the command system (see `ideas/kanban/app-architecture.md`).

- Register an AI panel command scope at the window layer:
  - `ai.toggle` — show/hide the panel (drives `AiPanelContainer` open-state)
  - `ai.focus` — move focus into the panel
  - `ai.newChat` — start a fresh stateless ACP session, clearing the conversation
  - `ai.model` — change model (`:ai model <name>`, autocomplete from `ai_list_models`)
  - `ai.cancel` — stop generation (`cancel` on the ACP client; available only while streaming)
- Add the command definitions to the appropriate builtin command YAML and the command implementations, following the existing pattern (`swissarmyhammer-commands` / `swissarmyhammer-kanban`, `compose_registry!`).
- Add keybindings for the vim / cua / emacs keymaps, consistent with the rest of the app.

## Acceptance Criteria
- [ ] `ai.toggle`, `ai.focus`, `ai.newChat`, `ai.model`, `ai.cancel` are registered and resolve through the scope chain at the window layer.
- [ ] The commands appear in the command palette with keybindings per keymap mode.
- [ ] `ai.toggle` shows/hides the panel; `ai.newChat` starts a fresh session; `ai.cancel` is available only while streaming.
- [ ] `cargo build` / `npm run build` succeed.

## Tests
- [ ] Command-resolution tests: each `ai.*` command resolves to its handler from the window scope.
- [ ] Test `ai.toggle` flips panel open-state and `ai.newChat` resets the conversation/session.
- [ ] Test `ai.cancel` is unavailable when idle, available while streaming.
- [ ] Relevant `cargo test` / `npm test` suites are green.

## Workflow
- Use `/tdd` — write the command-resolution and behavior tests first.