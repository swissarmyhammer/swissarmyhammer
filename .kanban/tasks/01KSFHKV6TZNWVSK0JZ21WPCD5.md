---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: 'AI panel: right-align the copy/retry action bar on user prompts'
---
## What

In the kanban AI panel, the per-message **copy** (and **retry**) buttons render flush-left underneath a user prompt, even though the user prompt bubble is right-aligned — so the action buttons look detached/off to the left of their message.

### Root cause

`MessageActionBar` in `apps/kanban-app/ui/src/components/ai-panel.tsx` renders `<MessageActions>` (from `apps/kanban-app/ui/src/components/ai-elements/message.tsx`, classed `flex items-center gap-1`) as a child of the `Message` flex **column**. For a user message:

- `Message` is `flex w-full max-w-[95%] flex-col … is-user ml-auto justify-end`, and
- `MessageContent` is `group-[.is-user]:w-fit group-[.is-user]:ml-auto` — i.e. a right-aligned, fit-width bubble.

But `MessageActions` is a column child with the default cross-axis `stretch`, so it spans the full message width and its buttons start at the **left** edge. Assistant messages are left-aligned content, so their action bar looks correct; only user messages are visually wrong.

### Fix

In `MessageActionBar` (`ai-panel.tsx`), make the action bar right-align for user messages by passing a `justify-end` class to `<MessageActions>` when `message.role === "user"`. `isUser` is already computed in that function, and `MessageActions` already merges an incoming `className` via `cn`, so this is a one-line, role-conditional class — e.g. `className={isUser ? "justify-end" : undefined}`. Assistant messages keep their default left alignment.

Do not change `ai-elements/message.tsx` (it is a shared/vendored AI Elements primitive); scope the fix to our call site in `ai-panel.tsx`.

## Acceptance Criteria

- [ ] On a **user** message, the copy + retry buttons are right-aligned — flush with the right edge of the message column, under the right-aligned prompt bubble (the `MessageActions` container carries `justify-end`).
- [ ] On an **assistant** message, the action bar stays left-aligned (no `justify-end`) — unchanged behavior.
- [ ] No change to `ai-elements/message.tsx`; the fix lives in `ai-panel.tsx`'s `MessageActionBar`.

## Tests

- [ ] In `apps/kanban-app/ui/src/components/ai-panel.test.tsx`, add a test that drives a user prompt through the existing `mockHarness`/`AiPanel` send-prompt flow (type into the composer + click submit, as the existing session tests do), then locates the "Copy message" button (`getByRole("button", { name: /copy message/i })`) and asserts its enclosing `MessageActions` container (`closest("div")` wrapping the action buttons) has the `justify-end` class.
- [ ] Assert the **negative**: an assistant message's action bar container does **not** carry `justify-end` (drive/await an assistant text response via the harness, or assert on the assistant turn the harness emits).
- [ ] `cd apps/kanban-app/ui && npx vitest run ai-panel.test.tsx` passes.
- [ ] Regression: the new user-message assertion fails before the `justify-end` change and passes after.

## Workflow

- Use `/tdd` — write the failing alignment test first (assert `justify-end` on the user action bar), watch it fail, then add the role-conditional class to make it pass. #bug #ux #kanban-app