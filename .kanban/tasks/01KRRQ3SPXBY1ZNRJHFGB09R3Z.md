---
assignees:
- claude-code
depends_on:
- 01KRRN6N593QQAA4RXZ2RBC1PF
position_column: todo
position_ordinal: 8c80
project: ai-panel
title: AI panel CM6 composer and bottom-bar AI status
---
## What
Finish the AI panel's text-editor and status integration.

- Make the panel's composer (the `PromptInput` text area) a CodeMirror 6 instance using the app's keymap (vim / emacs / CUA) — consistent with every other text input in the app ("CM6 everywhere", `ideas/kanban/app-architecture.md`). Not a plain `<textarea>`.
- Show AI status in the bottom bar: idle / streaming / error, sourced from the conversation store's turn status (the `sessionUpdate` task).

## Acceptance Criteria
- [ ] The composer is a CM6 instance honoring the active keymap; keymap motions work inside it.
- [ ] The bottom bar reflects AI status (idle / streaming / error).
- [ ] `npm run build` in `apps/kanban-app/ui` succeeds.

## Tests
- [ ] Component test: the composer is CM6 and a keymap motion works inside it.
- [ ] Component test: the bottom bar shows `streaming` during a prompt, `idle` after, `error` on failure.
- [ ] `npm test` in `apps/kanban-app/ui` is green.

## Workflow
- Use `/tdd` — write the composer-keymap and bottom-bar-status tests first.