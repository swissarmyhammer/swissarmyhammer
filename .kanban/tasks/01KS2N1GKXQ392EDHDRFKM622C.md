---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8980
project: ai-panel
title: 'AI panel: remove the "+ New conversation" button from the composer'
---
## What

The composer renders a ghost "+ New conversation" button above the prompt input as soon as the conversation has at least one message. The user does not want this button — the only way to start a fresh conversation should remain the `ai.newChat` command (keyboard shortcut / command palette). Remove the button entirely.

Files and locations:

- `apps/kanban-app/ui/src/components/ai-panel.tsx`
  - `ComposerArea` (~line 912-…) renders the button inside `{hasMessages && (...)}` (~line 926-938) using the `<Button>` + `<PlusIcon>` + label "New conversation". Delete that whole conditional block.
  - Remove the now-unused `hasMessages` and `onNewConversation` props from `ComposerAreaProps` (lines ~875, 888) and from the destructured parameter list (lines ~914, 922).
  - Update both `ComposerArea` call sites to stop passing those props:
    - `~line 342, 349` (the empty/disabled placeholder render path)
    - `~line 511, 518` (the live render path)
  - Update the doc comment on `ComposerArea` (~line 891-911) to drop the now-stale paragraph about the "New conversation" button.
- Keep the `newConversation` callback from `useConversation` and its registration into the `ai/commands.ts` registry (`registerAiCommandHandlers({ newChat: newConversation, ... })` ~line 406-413). The `ai.newChat` command remains the supported way to reset the conversation; only the in-composer button goes away.
- `apps/kanban-app/ui/src/components/ai-panel.test.tsx`
  - Delete the test `"hides 'New conversation' on an empty panel and reveals it after a message"` (~line 317).
  - Delete the test `"'New conversation' clears the message log"` (~line 364) — the reset behavior is still exercised through the `ai.newChat` command path elsewhere; if no existing test covers that path, add a replacement test that drives `ai.newChat` through the command registry and asserts the message log clears.

## Acceptance Criteria
- [x] The "+ New conversation" button no longer renders in the AI panel composer under any conversation state (empty or non-empty).
- [x] The `ai.newChat` command still resets the conversation (the registry handler is unchanged).
- [x] `ComposerArea` no longer accepts `hasMessages` or `onNewConversation` props; no caller passes them.
- [x] No dead imports remain in `ai-panel.tsx` (e.g. `PlusIcon` if its only use was this button — verify and remove if so).

## Tests
- [x] In `apps/kanban-app/ui/src/components/ai-panel.test.tsx`, remove the two button-specific tests listed above; if no other test covers the `ai.newChat`-driven reset path, add one that invokes the registered `ai.newChat` handler and asserts the message log is cleared.
- [x] Add or update a test asserting that after sending a message the composer does NOT contain a button with accessible name `/new conversation/i` (regression guard).
- [x] Run `cd apps/kanban-app/ui && npx vitest run src/components/ai-panel.test.tsx` — all green.
- [x] Run `cd apps/kanban-app/ui && npx tsc --noEmit` — clean (catches dead-prop / dead-import fallout).

## Workflow
- Use `/tdd` — write the failing "button is never present" regression test first, then delete the button and props.

## Implementation Notes

- TDD followed: added the regression-guard test first and confirmed it failed because the button was rendering; then removed the button and the test went green.
- The existing test `"the ai.newChat handler clears the conversation/session"` in the `AiPanel: ai.* command integration` describe block already exercises the `ai.newChat`-driven reset path via `triggerAiNewChat()`, so no additional test was needed for that flow — the two old button-specific tests were replaced with a single combined regression-guard test that asserts the button is absent both before and after sending a message.
- Removed `PlusIcon` from the `lucide-react` import (only the deleted button used it). `Button` is still used elsewhere in the file and kept.
- Test results: `vitest run src/components/ai-panel.test.tsx` — 11 passed, 0 failed. `tsc --noEmit` — clean.
