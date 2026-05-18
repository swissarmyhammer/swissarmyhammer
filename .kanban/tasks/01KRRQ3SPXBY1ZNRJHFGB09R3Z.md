---
assignees:
- claude-code
depends_on:
- 01KRRN6N593QQAA4RXZ2RBC1PF
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffb80
project: ai-panel
title: AI panel CM6 composer and bottom-bar AI status
---
## What
Finish the AI panel's text-editor and status integration.

- Make the panel's composer (the `PromptInput` text area) a CodeMirror 6 instance using the app's keymap (vim / emacs / CUA) — consistent with every other text input in the app ("CM6 everywhere", `ideas/kanban/app-architecture.md`). Not a plain `<textarea>`.
- Show AI status in the bottom bar: idle / streaming / error, sourced from the conversation store's turn status (the `sessionUpdate` task).

## Acceptance Criteria
- [x] The composer is a CM6 instance honoring the active keymap; keymap motions work inside it.
- [x] The bottom bar reflects AI status (idle / streaming / error).
- [x] `npm run build` in `apps/kanban-app/ui` succeeds.

## Tests
- [x] Component test: the composer is CM6 and a keymap motion works inside it.
- [x] Component test: the bottom bar shows `streaming` during a prompt, `idle` after, `error` on failure.
- [x] `npm test` in `apps/kanban-app/ui` is green.

## Workflow
- Use `/tdd` — write the composer-keymap and bottom-bar-status tests first.

## Implementation Notes

### Composer — CM6 via the app's shared `TextEditor` primitive
The composer was a plain `<textarea>` (`PromptInput` / `PromptInputTextarea` from the vendored AI Elements). It is now a CodeMirror 6 instance built on **`@/components/fields/text-editor` `TextEditor`** — the exact pure-CM6 primitive every other text input in the app wraps (`FilterEditor`, `MarkdownEditorAdapter`, `InlineRenameEditor`, `QuickCapture`). `TextEditor` reads the active keymap from `useUIState().keymap_mode` via `cm-keymap.ts`'s `keymapExtension`, so vim / emacs / CUA motions all work inside the composer for free — no bespoke CM6 setup.

New View `AiPromptComposer` (`apps/kanban-app/ui/src/components/ai-prompt-composer.tsx`) wraps `TextEditor` with the chat-composer policy:
- **Enter-submit keymap** — a `Prec.highest` `keymap.of([{ key: "Enter", run }])` extension supplied via `TextEditor`'s `extensions` prop (the same way `FilterEditor` / the markdown adapter supply their own submit keymaps). Plain `Enter` submits the trimmed buffer (no-op on an empty buffer); `Shift-Enter` is left unbound so it falls through to CM6's default and inserts a newline — multi-line prompts preserved. Keymap-agnostic, mirroring the prior textarea's "Enter sends, Shift+Enter newline".
- **Disabled state** — `EditorView.editable.of(!disabled)` extension makes the CM6 content DOM `contenteditable="false"` (CM6 has no `disabled` attribute).
- **Accessible name** — `EditorView.contentAttributes.of({ "aria-label": "Message the AI agent" })` keeps the content DOM addressable; CM6 already sets `role="textbox"`.
- **Submit/stop button** — preserved: a submit button between turns, a stop button (`onCancel`) while streaming.

`ComposerArea` in `ai-panel.tsx` now renders `AiPromptComposer` instead of `PromptInput`; the `AiPanelFocusScope moniker="ui:ai-panel.composer"` focus-scope/spatial-nav wiring from task `01KRRN6N593QQAA4RXZ2RBC1PF` is unchanged. `AiPanelConversation`'s `sendPrompt` / `cancel` wiring is unchanged (`handleSend` now takes a plain string instead of a `PromptInputMessage`).

### AI status reaches the bottom bar via the `ai/commands.ts` module store
The bottom bar (`ModeIndicator`) needs the conversation's turn status, but `ModeIndicator` lives in the window layer far from the AI panel tree. This reuses the established `ai/commands.ts` module-registry pattern (the same seam already bridging `ai.cancel`'s streaming gate):

- `ai/commands.ts` gained an `aiStatus` store (`idle | streaming | error`, the full `ConversationStatus`) as the **single source of truth**, with `setAiStatus` / `subscribeAiStatus`. The pre-existing `aiStreaming()` boolean is now a derived view (`aiStatus() === "streaming"`); `subscribeAiStreaming` aliases `subscribeAiStatus` (one shared subscriber set); `setAiStreaming` is a back-compat shim over `setAiStatus`.
- `AiPanelConversation` reports the full ACP turn status into the store via `setAiStatus(status)` (was `setAiStreaming(status === "streaming")`), so the `error` state is no longer flattened away. Resets to `idle` on unmount.
- `ModeIndicator` reads it with `useSyncExternalStore(subscribeAiStatus, aiStatus, aiStatus)` and renders an `AiStatusIndicator` (icon + label + tone) in its right slot.
- `ModeIndicator` previously returned `null` outside vim mode. It now **always renders** so the AI status is visible in every keymap; only the center vim-style mode label stays vim-gated.

### Files changed
- `apps/kanban-app/ui/src/ai/commands.ts` — added the `aiStatus` / `setAiStatus` / `subscribeAiStatus` status store; `aiStreaming` derived, `setAiStreaming` a shim.
- `apps/kanban-app/ui/src/components/ai-prompt-composer.tsx` (new) — the CM6 composer View.
- `apps/kanban-app/ui/src/components/ai-panel.tsx` — `ComposerArea` renders `AiPromptComposer`; `setAiStatus` instead of `setAiStreaming`; `handleSend` takes a string.
- `apps/kanban-app/ui/src/components/mode-indicator.tsx` — bottom-bar AI status indicator; always renders.
- `apps/kanban-app/ui/src/components/ai-panel-container.tsx` — `ai.focus` locates the composer by `role="textbox"` + `aria-label` (the CM6 content DOM) instead of `<textarea>`.

### Tests
- `apps/kanban-app/ui/src/components/ai-prompt-composer.test.tsx` (new) — 8 tests: composer is a real `.cm-editor` (not a `<textarea>`); a vim `0`+`x` motion works inside it; an emacs `Ctrl-A` motion works inside it; Enter submits; Shift+Enter inserts a newline; empty buffer does not submit; stop button cancels while streaming; disabled = non-editable.
- `apps/kanban-app/ui/src/components/mode-indicator.test.tsx` (new) — 4 tests: bar shows `streaming` during a prompt, `idle` after, `error` on failure; the bar renders outside vim mode so the AI status is visible.
- `apps/kanban-app/ui/src/ai/commands.test.ts` — added 5 tests for the new `aiStatus` store (defaults, the three states, the `aiStreaming` projection, the `setAiStreaming` shim, the shared subscriber set).
- Updated for the CM6 contract (composer is no longer a `<textarea>`): `ai-panel.test.tsx` (disabled-composer asserts `contenteditable="false"`), `ai-panel-container.test.tsx` (`ai.focus` targets the CM6 content DOM), `ai-panel.spatial.test.tsx` (per-message-action test types into the CM6 content DOM).

### Verification (actual output)
- `npm run build` (`tsc && vite build`) — exit 0, succeeds.
- `npm test` (`tsc --noEmit && vitest run`) — `Test Files 4 failed | 245 passed (249)`, `Tests 3 failed | 2268 passed | 35 skipped`. All 3 failing tests / 4 failing files are the documented pre-existing failures, NOT introduced here: the 3 stale-crate-path fixture suites of `01KRS426Q36ZN3DYBX2S0AS82T` (`slugify.parity.node.test.ts`, `editor-save.test.tsx`, `board-integration.browser.test.tsx`) and the CodeBlock/Shiki flake of `01KRVG4QSXPQ2FW5SG61M8EHAP` (`ai-elements.smoke.test.tsx`). Zero NEW failures. The 17 new/updated tests across `ai-prompt-composer.test.tsx`, `mode-indicator.test.tsx`, `commands.test.ts` and the touched AI-panel suites all pass.

## Review Findings (2026-05-18 13:46)

### Nits
- [x] `apps/kanban-app/ui/src/components/ai-prompt-composer.tsx:24-25,53` — The file docstring and `buildEnterSubmitExtension` docstring claim the Enter-submit keymap is supplied "the same way `FilterEditor` and the markdown adapter supply their own" — but `FilterEditor` (`filter-editor.tsx:38,315-325`) and `MarkdownEditorAdapter` (`fields/registrations/markdown.tsx:27,72-83`) both route through the shared `buildSubmitCancelExtensions` helper (`@/lib/cm-submit-cancel.ts`), the codebase's single vim-mode-aware submit/cancel factory. `AiPromptComposer` instead hand-rolls a bespoke `Prec.highest` `keymap.of` inline. The composer's policy (Enter-always-submit + Shift-Enter-newline, multi-line) genuinely differs from every current `buildSubmitCancelExtensions` caller (all single-line), so a bespoke keymap is defensible — but the helper's `alwaysSubmitOnEnter` branch already produces exactly this `Prec.highest` Enter binding and additionally guards on `completionStatus` (yield to autocomplete). The bespoke version drops that guard; harmless today because the composer has no autocomplete (`BASIC_SETUP.autocompletion: false`, no mention extensions), but a future maintainer adding autocomplete would miss it. Suggest either reusing `buildSubmitCancelExtensions` (it can express this policy) or correcting the docstrings to say the composer uses a deliberately bespoke keymap and why, rather than claiming consistency that does not hold.
  - **Resolution (2026-05-18): KEEP the bespoke keymap; corrected the docstrings.** Investigated `buildSubmitCancelExtensions` and both callers. The helper genuinely *cannot* express the composer's policy: (1) it always wires an Escape→cancel binding, but the composer needs Escape to stay a plain vim insert→normal toggle (its cancel is the stop button — there is no cancel callback to pass); (2) its CUA/emacs Enter binding is gated on `singleLine`, not `alwaysSubmitOnEnter`, so a multi-line composer (`singleLine: false`) would get NO Enter-submit in CUA/emacs mode, while `singleLine: true` would suppress vim insert-mode newline insertion and break multi-line prompts — no flag combination yields "Enter always submits, Shift-Enter always newlines". The bespoke `Prec.highest` keymap is therefore required. Fixed the file docstring and `buildEnterSubmitExtension` docstring: removed the false "exactly as `FilterEditor` and the markdown adapter supply their own" claim and added a "Why a bespoke keymap" section explaining both helper limitations and noting the deliberate omission of the `completionStatus` autocomplete-yield guard (the composer has no autocomplete; a maintainer adding one must re-introduce it).
- [x] `apps/kanban-app/ui/src/components/ai-prompt-composer.tsx:64-83` — On an empty/whitespace-only buffer the Enter binding's `run` returns `false`, letting the keystroke fall through to CM6's default `insertNewline`. Pressing Enter on an empty composer therefore inserts a blank line, and repeated Enter presses accumulate blank lines (the empty-buffer test confirms the buffer becomes `"\n"` after one Enter). It never submits garbage — the trimmed-empty guard holds — and the Implementation Notes call the fall-through deliberate, so this is cosmetic only. A true no-op (`return true` without invoking submit) would match the "stray Enter on an empty composer is a no-op" intent stated in the same docstring more faithfully.
  - **Resolution (2026-05-18): made empty-Enter a true no-op.** The empty/whitespace-only branch of `buildEnterSubmitExtension`'s Enter `run` now returns `true` instead of `false`, swallowing the keystroke so it neither submits nor falls through to `insertNewline`. Repeated Enter on an empty composer no longer accumulates blank lines. Updated the `buildEnterSubmitExtension` docstring and inline comment to match. `Shift-Enter` is still unbound and inserts a newline as before; multi-line non-empty input is unchanged. Updated the test (`ai-prompt-composer.test.tsx`, renamed "Enter on an empty buffer is a true no-op — no submit, no blank line") to press Enter three times and assert the buffer stays `""`, not just that `onSend` was not called.