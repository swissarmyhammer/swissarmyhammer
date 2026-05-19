---
assignees:
- claude-code
position_column: todo
position_ordinal: '9580'
project: ai-panel
title: 'AI panel: rework the composer to the AI Elements PromptInput layout'
---
## What

The AI panel composer is hand-rolled instead of composed from the vendored AI Elements `PromptInput` family (`apps/kanban-app/ui/src/components/ai-elements/prompt-input.tsx`). Three symptoms, one root cause:

1. **Model selector is in the wrong place.** It is a bespoke header `DropdownMenu` in `AiPanelHeader` (`apps/kanban-app/ui/src/components/ai-panel.tsx`). The AI Elements standard puts model selection *in the input area* — `prompt-input.tsx` ships a `Select`-based model picker (`PromptInputSelect`, `PromptInputSelectTrigger`, `PromptInputSelectValue`, `PromptInputSelectContent`, `PromptInputSelectItem`) meant to sit in the composer footer.
2. **Double border around the input.** `ComposerArea` wraps the composer in `<div className="border-t p-2">` and `AiPromptComposer` (`apps/kanban-app/ui/src/components/ai-prompt-composer.tsx`) then nests the CM6 editor in its own `<div className="rounded-md border bg-background px-2 py-1.5">` well — two stacked borders read as a doubled edge.
3. **CM6 does not auto-expand.** The CM6 editor well is content-height; it should grow to fill the panel's available vertical space so the prompt area uses the room it has.

The composer (`AiPromptComposer`) is correctly a CodeMirror 6 editor — "CM6 everywhere" is an architecture rule (`ideas/kanban/app-architecture.md`). Keep CM6. Do **not** swap in `PromptInputTextarea` (a plain `<textarea>`). The fix is to adopt the AI Elements composer *layout* — single bordered container, model select + submit in a footer toolbar — while keeping the CM6 `TextEditor` as the actual text input.

### Approach

**Model selector → composer footer (`ai-panel.tsx`, `ai-prompt-composer.tsx`):**
- Remove the model-selector `DropdownMenu` block from `AiPanelHeader`. The header keeps the "AI" title (and the collapse button from task `01KS09CQF2XSG25RHA0A38M625`, if landed).
- Thread `models`, `modelId`/`selectedModel`, and `onSelectModel` from `AiPanel` down through `AiPanelConversation` into `ComposerArea` and into the composer.
- Render the model picker in the composer footer using the AI Elements `PromptInputSelect*` components. Preserve the spatial-nav focus leaf — the trigger must still register the `ui:ai-panel.model-selector` moniker (currently via `AiPanelPressable`), now under the composer scope.
- Keep the existing model-availability behavior: unavailable models are disabled options that still surface their `hint`.

**Single bordered container:**
- Give the composer one bordered container (matching the AI Elements `PromptInput` shell: a single `rounded-*`/`border` box holding the CM6 body and the footer toolbar). Drop the redundant border — either the `ComposerArea` section `border-t` or the inner `rounded-md border` well, not both.

**CM6 auto-expand:**
- The CM6 editor should flex to fill available height: the composer body gets `flex-1`/`min-h-0`, and the CM6 `.cm-editor` / `.cm-scroller` fill it (`h-full`) so the prompt area grows with the panel rather than staying content-height. The footer (model select + submit) stays pinned at the bottom of the container.

Keep the submit/stop button behavior (`PromptInputSubmit`-style: send when idle, stop when streaming) and the Enter-submit / Shift-Enter-newline keymap unchanged.

## Acceptance Criteria
- [ ] The model selector is rendered in the composer (input area), not in the panel header; `AiPanelHeader` no longer contains a model dropdown.
- [ ] The model picker uses the AI Elements `PromptInputSelect*` components; unavailable models are disabled and show their hint.
- [ ] Selecting a model still reports the choice via `onSelectModel` and starts a fresh ACP session (existing remount-on-`modelId` behavior preserved).
- [ ] The composer has a single border around the input — no doubled/stacked border edge.
- [ ] The CM6 editor expands to fill the composer's available vertical space; the footer toolbar stays pinned at the bottom.
- [ ] The model-selector trigger is still a spatial-nav focus leaf (`ui:ai-panel.model-selector`).
- [ ] Enter submits / Shift-Enter inserts a newline; the submit button becomes a stop control while streaming — all unchanged.

## Tests
- [ ] Update `apps/kanban-app/ui/src/components/ai-panel.test.tsx` "AiPanel: model selector" tests: the selector is now found in the composer, not the header — assert the picker lists models, disables unavailable ones with their hint, and that selecting one calls `onSelectModel`.
- [ ] Add a test asserting the model picker is NOT inside the `<header>` / is within the composer region.
- [ ] In `apps/kanban-app/ui/src/components/ai-prompt-composer.test.tsx`, add/extend tests for the footer model select and confirm the existing submit/stop and Enter-keymap tests still pass.
- [ ] Add an assertion that the composer renders a single bordered container (no nested `border` element inside another `border` element for the input well).
- [ ] Run `cd apps/kanban-app/ui && npx vitest run src/components/ai-panel.test.tsx src/components/ai-prompt-composer.test.tsx src/components/ai-panel.spatial.test.tsx` — all green.

## Workflow
- Use `/tdd` — write the failing model-selector-location and single-border tests first, then implement.
