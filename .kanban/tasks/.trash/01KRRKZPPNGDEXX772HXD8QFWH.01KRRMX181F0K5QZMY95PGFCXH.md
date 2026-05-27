---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: 8c80
project: ai-panel
title: AI panel command scope and keybindings
---
## What
Make the AI panel a first-class citizen of the command system (see `ideas/kanban/app-architecture.md`).

- Register an AI panel command scope at the window layer with commands:
  - `ai.toggle` — show/hide the panel (`ai.toggle` drives the `AiPanelContainer` open-state)
  - `ai.focus` — move focus into the panel
  - `ai.newChat` — start a fresh stateless session, clearing the conversation
  - `ai.model` — change model (`:ai model <name>` pattern, autocomplete from `ai_list_models`)
  - `ai.cancel` — stop generation (active while streaming)
- Add the command definitions to the appropriate builtin command YAML for the kanban app and the command implementations (follow the existing command registration pattern — `swissarmyhammer-kanban`/`swissarmyhammer-commands`, `compose_registry!`).
- Add keybindings for the vim/cua/emacs keymaps consistent with the rest of the app.

Spec: `ideas/kanban/ai_panel.md` — Phase 5 "Command scope". Pattern reference: `ideas/kanban/app-architecture.md` (command scopes, palette, keybindings).

## Acceptance Criteria
- [ ] `ai.toggle`, `ai.focus`, `ai.newChat`, `ai.model`, `ai.cancel` are registered and resolve through the scope chain at the window layer.
- [ ] The commands appear in the command palette and have keybindings for each keymap mode.
- [ ] `ai.toggle` shows/hides the panel; `ai.newChat` starts a fresh session; `ai.cancel` is available only while streaming.
- [ ] `cargo build` / `npm run build` succeed.

## Tests
- [ ] Command-resolution tests: each `ai.*` command resolves to its handler from the window scope.
- [ ] Test that `ai.toggle` flips the panel open-state and `ai.newChat` resets the conversation/session.
- [ ] Test that `ai.cancel` is unavailable when idle and available while streaming.
- [ ] Relevant `cargo test` / `npm test` suites are green.

## Workflow
- Use `/tdd` — write the command-resolution and behavior tests first.