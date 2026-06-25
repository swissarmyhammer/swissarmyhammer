---
assignees:
- claude-code
position_column: todo
position_ordinal: fd80
project: ai-panel
title: AI panel model selector overflows on long model names instead of truncating with ellipsis
---
## What

**Bug**: In the AI panel composer footer, a long model label (e.g. the Claude Code CLI model name) makes the model-selector trigger grow to its full content width and overflow the panel horizontally, instead of truncating with an ellipsis (`…`).

**Root cause** — the footer model picker is `ComposerModelSelect` in `apps/kanban-app/ui/src/components/ai-prompt-composer.tsx` (around line 311), which renders a `PromptInputSelectTrigger` (→ shadcn `SelectTrigger` in `apps/kanban-app/ui/src/components/ui/select.tsx:25`). The base `SelectTrigger` className (select.tsx:38) is `flex w-fit … whitespace-nowrap` and applies `line-clamp-1` to the value slot (`*:data-[slot=select-value]:line-clamp-1`). But:
- `w-fit` sizes the trigger to its content, and there is no `max-w` or `min-w-0`.
- As a flex item in the footer toolbar (`<div className="flex items-center justify-between gap-2 …">`, ai-prompt-composer.tsx:664), the trigger's default `min-width: auto` is its content width, so the flex item never shrinks below the full label width.

With nothing allowing the trigger to shrink, `line-clamp-1` never has a constrained width to clamp against, so the label renders full-width and pushes the footer past the panel edge.

### Fix approach (call-site only — do NOT touch shared `SelectTrigger`/`PromptInputSelectTrigger`, used elsewhere)

In `apps/kanban-app/ui/src/components/ai-prompt-composer.tsx`:
- [ ] Add `min-w-0` to the `PromptInputSelectTrigger` in `ComposerModelSelect` (pass via its `className` — `PromptInputSelectTrigger` forwards `className` through `cn(...)`). This lets the flex item shrink below content width so the existing `line-clamp-1` on the value slot clamps and shows the ellipsis. Optionally also cap with a `max-w` if needed for the narrow dock.
- [ ] Add `shrink-0` to the footer submit/stop `<button>` (ai-prompt-composer.tsx:681 className block) so the fixed `size-7` action button is never compressed when the selector shrinks.
- [ ] If the `AiPanelPressable asChild` wrapper interferes with class merging onto the trigger, apply `min-w-0` directly on the element that becomes the `role="combobox"` button.

### Notes
- The UI test project runs in **real Chromium via Playwright** with Tailwind applied (`apps/kanban-app/ui/vite.config.ts`), so a real layout/overflow assertion is possible — no class-only proxy needed.

## Acceptance Criteria
- [ ] With a pathologically long model label and the composer constrained to a narrow width (e.g. 320px, the dock width), the footer toolbar does not overflow its container: the toolbar's `scrollWidth` is `<=` the container's `clientWidth`.
- [ ] The model-selector value is clamped (the value element's `scrollWidth > clientWidth`), i.e. the label is truncated rather than shown in full.
- [ ] The submit/stop action button keeps its `size-7` footprint (not compressed) when the label is long.
- [ ] Short model labels still render fully (no regression to existing footer-select tests).

## Tests
- [ ] In `apps/kanban-app/ui/src/components/ai-prompt-composer.test.tsx`, add a test under the existing `describe("AiPromptComposer — footer model select", …)` block: render `<AiPromptComposer …>` inside a `<div style={{ width: 320 }}>` with a model whose `label` is a long string (e.g. `"claude-opus-4-8[1m] Anthropic Claude Code CLI — very long model display name"`) and `selectedModel` set to it. Query the footer toolbar element and assert `toolbar.scrollWidth <= toolbar.clientWidth` (no horizontal overflow), and query the `role="combobox"` trigger's value element (`[data-slot='select-value']`) and assert `valueEl.scrollWidth > valueEl.clientWidth` (clamped). This fails before the fix (trigger overflows) and passes after.
- [ ] Keep/verify the existing footer-select tests pass: `screen.getByRole("combobox", { name: /claude code/i })` still resolves for the standard `MODELS` fixture.
- [ ] Run: `cd apps/kanban-app/ui && npm test -- ai-prompt-composer` — all pass.
- [ ] Run the full UI suite: `cd apps/kanban-app/ui && npm test` — no regressions.

## Workflow
- Use `/tdd` — write the narrow-width overflow test first (it fails: the toolbar overflows / the value is not clamped), then add `min-w-0` to the trigger and `shrink-0` to the action button so it passes. #ui