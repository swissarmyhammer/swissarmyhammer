---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffa880
title: 'AI panel: assistant tool-call folds should render full-width'
---
## What

In the AI panel, the collapsible tool-call cards ("tool folds") render narrow — sized to their own content — instead of spanning the conversation column. Per the AI Elements layout, assistant block content (tool cards, the plan, etc.) should be full-width; only the *user* message is a fit-width bubble.

### Root cause (researched)

The tool card itself is already `w-full` (`apps/kanban-app/ui/src/components/ai-elements/tool.tsx:27` — `Tool` = `Collapsible` with `not-prose mb-4 w-full rounded-md border`). The constraint is its wrapper: `MessageContent` in `apps/kanban-app/ui/src/components/ai-elements/message.tsx:43-59` is `w-fit max-w-full min-w-0 …` **unconditionally**. `w-fit` sizes the content box to its intrinsic content, so a `w-full` child resolves to that shrunk width — the tool fold ends up as wide as its header/JSON, not the column.

`w-fit` is correct for the **user** bubble (`group-[.is-user]:rounded-lg group-[.is-user]:bg-secondary …` — the bubble hugs the text). It is wrong for **assistant** content, which is flat block content (`group-[.is-assistant]:text-foreground`) and should fill the message width.

`Message` (message.tsx:30-39) is `group flex w-full max-w-[95%] …` with `is-user` / `is-assistant` group classes, so width can be switched per role via the existing group selectors.

### Approach (touch-up)

1. `apps/kanban-app/ui/src/components/ai-elements/message.tsx` — in `MessageContent`, make the width role-conditional instead of a blanket `w-fit`: keep `w-fit` for the user bubble (`group-[.is-user]:w-fit`) and use `w-full` for assistant content (`group-[.is-assistant]:w-full`). Preserve `max-w-full min-w-0 overflow-hidden` and the existing user-bubble padding/background classes. Net effect: assistant tool folds (and plan/text) span the assistant message width; user bubbles still hug their text.
2. No change needed to `tool.tsx` (already `w-full`). The tool card root is selectable via `data-slot="collapsible"` (from `ui/collapsible.tsx`) for the test.

### Consumers to keep working

`MessageContent` is used by `ai-panel.tsx` (`ConversationMessageView`, line ~611) and `ai-elements/ai-elements.smoke.test.tsx`. The user-bubble path must be unchanged.

### Non-goals

- Do not change the `Message` `max-w-[95%]` cap, the user-bubble styling, the `Tool` component, or any tool-card internals.
- Do not restyle reasoning/plan cards beyond the width they inherit from this fix.

## Acceptance Criteria

- [x] An assistant message containing a tool-call card renders that card at (approximately) the full width of its assistant message region — not shrunk to the card's intrinsic content width.
- [x] A user message still renders as a fit-width bubble (its content box is narrower than the column when the text is short).
- [x] No regression to assistant text/reasoning/plan rendering or to the user-bubble background/padding.

## Tests

- [x] Extend `apps/kanban-app/ui/src/components/ai-panel.test.tsx` (browser project) — stream a `tool_call` update (a short tool name + small input/output) into an assistant message, then read `getBoundingClientRect()` for the assistant `Message` wrapper (`.is-assistant`) and the tool card (`.is-assistant [data-slot="collapsible"]`); assert the tool card width is ≈ the assistant message width (e.g. `toolRect.width >= messageRect.width - 2`). Render at a wide-enough panel/viewport that the tool's intrinsic content is clearly narrower than the column, so the assertion FAILS before the fix (card shrunk to content) and PASSES after.
- [x] Add a user-bubble guard (same file or `ai-elements.smoke.test.tsx`): a short user message's `MessageContent` width is strictly less than the conversation column width (still a fit bubble) — guards against the fix accidentally making user messages full-width.
- [x] `pnpm --filter ./apps/kanban-app/ui test ai-panel` and `... test ai-elements` green.
- [x] Full UI suite green: `pnpm --filter ./apps/kanban-app/ui test`.

## Workflow

- Use `/tdd` — write the failing full-width tool-card width assertion first (it should fail against the current `w-fit`), then make `MessageContent` assistant-width conditional to pass it.

### Implementation note (test environment)

The browser test project does **not** load `@tailwindcss/vite`, so utility classes carry no CSS during tests — every element falls back to `display: block; width: auto` and a pure `getBoundingClientRect()` width test cannot distinguish `w-fit` from `w-full`. Following the established codebase pattern (`app-layout.test.tsx`, the `*.spatial.test.tsx` suites), the two new tests install a small per-test Tailwind shim stylesheet (`installMessageWidthShim`) that translates exactly the width-determining utilities — including the role-conditional `group-[.is-*]:w-*` variants (matched via `[class~="…"]` attribute selectors under `.is-user` / `.is-assistant`) — into real CSS, and render the panel in a wide 1000px host. With the shim, fail-before is genuine: the assistant tool card measured 168px against a 948px message wrapper before the fix; full-width after.

#bug #ux