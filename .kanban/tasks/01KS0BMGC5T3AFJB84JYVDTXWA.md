---
assignees:
- claude-code
position_column: todo
position_ordinal: '9880'
project: ai-panel
title: 'AI panel: toggling the panel must not destroy the conversation'
---
## What

Collapsing and re-expanding the AI panel (the `ai.toggle` command / collapse button) loses the whole conversation. Toggling should only change panel visibility — the conversation and ACP session must survive.

Root cause: `AiPanelShell` in `apps/kanban-app/ui/src/components/ai-panel-container.tsx` **unmounts** its `children` when collapsed. The collapsed branch (`if (!open)`) returns a thin rail `<div>` that does not render `{children}` at all; only the expanded branch renders the hosted `<AiPanel>`. So collapsing unmounts `AiPanel` → `AiPanelConversation` → the `useConversation` hook, and `useConversation` persists nothing (`apps/kanban-app/ui/src/ai/conversation.ts`: "Nothing is persisted"). Re-expanding mounts a fresh `AiPanel` with an empty store and a brand-new ACP session.

The user already has a dedicated way to start fresh — the `ai.newChat` command / the composer's "New conversation" action. Toggling the panel must not also do that.

### Approach

Keep the hosted `AiPanel` mounted across collapse/expand — hide it with CSS instead of unmounting it.

In `AiPanelShell`:
- Always render `children` (the panel body) in the tree, regardless of `open`.
- When collapsed (`!open`): apply a `hidden` (`display: none`) class to the body so it takes no layout space, render the thin rail with the "Expand AI panel" button, and size the outer container to the rail width (`w-9`).
- When expanded (`open`): show the body, render the left-edge resize handle, and size the container to `width`.
- The rail and the body live under one always-mounted outer container so the body is never unmounted by a toggle.

This preserves the `useConversation` store and the live ACP session across any number of toggles. A genuine remount (board switch, app restart) still legitimately starts fresh — out of scope here.

Do not change `useConversation`, `newConversation`, the `ai.newChat` command, or the `ai.toggle` open-state persistence.

## Acceptance Criteria
- [ ] Collapsing then re-expanding the AI panel preserves the conversation messages and the ACP session — nothing is reset.
- [ ] The hosted `AiPanel` (`[data-slot='ai-panel']`) stays mounted in the DOM while the panel is collapsed; it is hidden, not removed.
- [ ] The collapsed rail still shows the "Expand AI panel" button and the panel still expands/collapses; the `ai.toggle` command still works and its open-state still persists per board.
- [ ] Starting a fresh conversation is still possible via `ai.newChat` / the "New conversation" action — that path is unchanged.

## Tests
- [ ] In `apps/kanban-app/ui/src/components/ai-panel-container.test.tsx`, update the existing `"collapses and expands; ..."` test: it currently asserts `[data-slot='ai-panel']` is `null` once collapsed (line ~197-200) — change it to assert the body is still present but hidden when collapsed (e.g. not visible / has the `hidden` class), and visible again when expanded.
- [ ] Add a test that drives a conversation (render a message into the panel), collapses via the toggle, re-expands, and asserts the message is still rendered — i.e. the conversation survived the toggle.
- [ ] Confirm the collapsed-state persistence assertions in that test still hold (storage `open: false` / `open: true`).
- [ ] Run `cd apps/kanban-app/ui && npx vitest run src/components/ai-panel-container.test.tsx` — all green.

## Workflow
- Use `/tdd` — write the failing "conversation survives a toggle" test first, then implement.
