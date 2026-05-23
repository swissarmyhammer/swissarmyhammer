---
assignees:
- claude-code
position_column: review
position_ordinal: '80'
title: 'AI composer: slash-command autocomplete from ACP `availableCommands`'
---
## What

The AI panel's prompt composer (`apps/kanban-app/ui/src/components/ai-prompt-composer.tsx`) had **no autocomplete**. When an ACP agent advertises slash commands via `session/update`'s `available_commands_update` (e.g. `/plan`, `/review`, `/finish`), typing `/` in the composer gave no completion menu. This reuses the shared mention-autocomplete component for `/` slash-command completions, fed by the live ACP `availableCommands`.

### Approach (all done)

1. **Slash-command flavor in `lib/cm-mention-autocomplete.ts`**: `CommandSearchResult` (`{ name, description }`), a `CompletionSearchResult` union + `isCommandResult` guard, `buildCommandOption` (label/apply `/<name>`, description in `info()`, no color dot), and an `openOnBarePrefix` option so a bare `/` lists every command. Empty results yield `null`. One shared source builder.
2. **`useCommandCompletionExtension` hook** in a dedicated lightweight `hooks/use-command-completion.ts` (no React context, so the composer doesn't transitively import the heavy entity-store/window-container chain). Heavy lifting stays in `lib/cm-mention-autocomplete.ts`.
3. **Composer wiring**: `availableCommands` prop (default `[]`); extension appended to `baseExtensions`; Enter-submit binding yields via `completionStatus(view.state) === "active"`. Docstring updated.
4. **Threading**: `AiPanelConversation` pulls `state.availableCommands` → `ComposerArea` → `AiPromptComposer`.

### Note on the Enter guard

Used `=== "active"` (not the helper's `!== null`): `activateOnTyping` transiently reports `"pending"` over ordinary prose, and a chat composer must still submit on Enter then.

## Acceptance Criteria

- [x] Typing `/` opens a completion menu listing every `availableCommands` entry; typing more filters by substring.
- [x] Each completion shows `/<name>` and its `description` in the info area. No color dot.
- [x] Accepting (Enter or click) inserts literal `/<name>`. No interception/transform.
- [x] Menu open: plain Enter accepts and does NOT submit. Menu closed: plain Enter submits. Shift-Enter always inserts a newline.
- [x] Escape open → closes menu (CM6); Escape closed → drills out via `buildSubmitCancelExtensions`.
- [x] No regressions to `#tag`/`@actor`/`^task` completions in the filter/markdown editors.
- [x] Empty `availableCommands` → typing `/` produces no menu, Enter submits.
- [x] Shared helpers remain the single completion-source assembly point; no parallel file.

## Tests

- [x] `cm-mention-autocomplete.test.ts`: `/` prefix cases (empty→null; label/apply `/${name}`; `info()` description, no dot; `openOnBarePrefix` auto-trigger; default bare-prefix suppression unchanged).
- [x] `hooks/__tests__/use-command-completion.test.ts`: hook opens/lists/filters the `/` menu; empty list → no extension.
- [x] `ai-prompt-composer.test.tsx`: `/` opens menu when non-empty, not when empty; Enter-with-menu accepts (buffer `/plan`) and doesn't submit; Enter-menu-closed submits; Shift-Enter newline.
- [x] `ai-panel.test.tsx`: streamed `available_commands_update` drives the composer's `/` menu.
- [x] Full UI suite green.

## Follow-up — manual verification fix (warm-up on model select)

Manual test surfaced that `/` showed nothing on a fresh chat. Root cause: the menu is driven by the live `availableCommands`, which the claude backend forwards (from the CLI init message) only when the session starts — and the session started lazily on the *first* `sendPrompt`. So a never-sent-to conversation had an empty command list.

Fix (chosen direction: **warm up on model select**):
- [x] Added `warmUp()` to the conversation API (`conversation.ts`): fire-and-forget eager session start; idempotent; shares one in-flight session with `sendPrompt` via a new `sessionPromiseRef` guard so a warm-up racing the first send never spawns two agents. `newConversation` clears the in-flight ref.
- [x] `AiPanelConversation` warms up via `useEffect` whenever `modelReady && messages.length === 0` — covers model-select mount AND re-warms after `ai.newChat` so `/` keeps working in a fresh chat.
- [x] Tests: `conversation.test.tsx` (warmUp folds init `available_commands_update` without a prompt; warmUp racing sendPrompt → one session); `ai-panel.test.tsx` (`/` works before any message; re-warms after `ai.newChat`). Init-time ambient updates added to both test harnesses' `startSession` to mirror the real agent.
- [x] Full UI suite green: 254 files, 2431 tests; `tsc --noEmit` clean.

Note: a local-llama model advertises no commands (`agent_trait_impl.rs`), so `/` is correctly empty there.

## Workflow

- `/tdd` throughout — failing test first, watched RED, GREEN, refactor — for every layer including the warm-up follow-up.