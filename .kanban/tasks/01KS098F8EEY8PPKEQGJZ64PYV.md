---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8180
project: ai-panel
title: 'AI panel: hide ''New conversation'' button on an empty conversation'
---
## What

On a fresh AI panel the "New conversation" button is always rendered in the composer (`ComposerArea`, `apps/kanban-app/ui/src/components/ai-panel.tsx`). This makes it look like a required first step — but it is not. `useConversation` (`apps/kanban-app/ui/src/ai/conversation.ts`) already creates the ACP session **lazily** on the first `sendPrompt` via `ensureSession`; the empty state already says "Send a message to start the conversation." The user should just type and chat — pressing "New conversation" first should never be necessary.

The "New conversation" button's only real job is to *reset* an existing conversation. When there are no messages there is nothing to reset, so the button is pure clutter and a misleading affordance.

Fix: render the "New conversation" button only when the conversation is non-empty.

- In `AiPanelConversation` (the component rendering `ComposerArea` around line 564 of `ai-panel.tsx`), `messages` is already in scope. Pass a `hasMessages: messages.length > 0` prop into `ComposerArea`.
- In `ComposerArea`, add `hasMessages: boolean` to `ComposerAreaProps` and only render the `<Button>` (the `<div className="mb-1 flex justify-end">` wrapper holding it) when `hasMessages` is true. The CM6 `AiPromptComposer` below it always renders, unchanged.
- Update the `ComposerArea` doc comment so it no longer describes the button as always present.

Do not change session creation, `newConversation`, or the empty-state copy — only the button's visibility.

## Acceptance Criteria
- [x] On a freshly opened AI panel with zero messages, the "New conversation" button is not in the DOM.
- [x] After at least one message exists in the conversation, the "New conversation" button is rendered and still resets the session when clicked.
- [x] The composer (CM6 prompt input) is rendered and usable on an empty panel — typing a prompt and sending it starts the conversation with no prior button press (existing lazy `ensureSession` behavior, unchanged).
- [x] No change to `useConversation`, `newConversation`, or the empty-state component.

## Tests
- [x] In `apps/kanban-app/ui/src/components/ai-panel.test.tsx`, add a test: a freshly rendered `AiPanel` with no messages does NOT show a "New conversation" button (`queryByRole("button", { name: /new conversation/i })` is null).
- [x] Add/extend a test: after a message is streamed in, the "New conversation" button appears.
- [x] Confirm the existing `"'New conversation' clears the message log"` test (line ~315) still passes — it sends a message first, so the button is visible there.
- [x] Run `cd apps/kanban-app/ui && npx vitest run src/components/ai-panel.test.tsx` — all green.

## Workflow
- Use `/tdd` — write the failing visibility tests first, then implement to make them pass.
