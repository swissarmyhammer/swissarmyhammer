---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8580
project: ai-panel
title: 'AI panel: model picker not keyboard-focusable, and no drill-out from the composer editor'
---
## What

Two follow-on keyboard-focus gaps in the AI composer, both the same class as the CM6 drill-in fix (task `01KS0WMB0MT0GVWMHF7BREGKWS`): the spatial-nav leaf is registered but does not actually hand the keyboard to the control.

### 1. Model picker is not keyboard-focusable

Jumping / navigating to the `ui:ai-panel.model-selector` leaf does not let the user keyboard the model picker. The `onPress` is a deliberate no-op; the spatial-nav leaf being focused does not put DOM focus on the trigger `<button>`, so Radix `Select` (which needs DOM focus on its trigger for Space/Enter/↑↓) is never reachable by keyboard.

Fix: give `ComposerModelSelect` a ref to the `PromptInputSelectTrigger` and have `onPress` call `.focus()` on it.

### 2. No drill-out from the CM6 editor — Escape doesn't release focus

Once the CM6 prompt has DOM focus, Escape doesn't release it: focus stays trapped, so `s` (jump) can't re-open the jump overlay.

Fix: route the composer's Escape through the **shared** `buildSubmitCancelExtensions` helper (`@/lib/cm-submit-cancel.ts`) — the exact mechanism every other CM6 editor in the app uses (filter formula bar, markdown field, command palette, single-/multi-select, date editor, perspective tab bar). The helper's vim path is a two-phase DOM-capture listener that correctly preempts `@replit/codemirror-vim`'s insert-mode Escape; its CUA/emacs path is a `Prec.highest` keymap. The composer supplies `singleLine: false` so the helper contributes only its Escape handling — the composer keeps its bespoke Enter-submit keymap. The drill-out callback (blur + `nav.focus(composerFq)`) is composed inside the composer scope by `ComposerEditorDrillOutWiring` and handed down via `ComposerEditorEscapeContext`.

## Approach

- **Model picker (`ai-prompt-composer.tsx`):** add `triggerRef` on `PromptInputSelectTrigger`; `AiPanelPressable.onPress` calls `triggerRef.current?.focus()`. Pointer click still opens (Radix's own handler); `.focus()` on the already-focused trigger is inert.
- **Drill-out (`ai-prompt-composer.tsx`):** adopt the shared `buildSubmitCancelExtensions` helper. Add `ComposerEditorDrillOutWiring` (provides the drill-out callback via `ComposerEditorEscapeContext`, mounted inside the `ui:ai-panel.composer` scope so it can read the composer FQM via `useOptionalFullyQualifiedMoniker`) and `ComposerEditorBody` (consumes the context and passes the callback as `onCancelRef`). In the no-spatial-stack unit-test path the FQM is `null`, the context value is `null`, and the helper's Escape handler is an inert no-op — same as the filter editor mounted bare.
- Do not change the Enter-submit / Shift-Enter keymap or the submit/stop control.

## Acceptance Criteria
- [x] Navigating / jumping to the model picker leaf puts DOM focus on the model-select trigger; Space/Enter/↑↓ then open and navigate the model listbox.
- [x] Activating the model picker by keyboard selects a model (still calls `onSelectModel`); pointer click still opens it.
- [x] Pressing Escape while the CM6 prompt has focus drills out: the editor loses DOM focus and kernel spatial focus returns to the `ui:ai-panel.composer` scope.
- [x] After that Escape, pressing `s` re-opens the jump overlay (focus is no longer trapped in the editor).
- [x] The Enter-submit / Shift-Enter keymap and the submit/stop control are unchanged.
- [x] The `AiPromptComposer` docstring is updated — Escape now drills out via the shared helper (the prior vim-toggle rationale removed).

## Tests
- [x] In `apps/kanban-app/ui/src/components/ai-panel.spatial.test.tsx`: a test that activating the model-picker leaf puts `document.activeElement` on the select trigger.
- [x] A test that firing Escape while the CM6 prompt has DOM focus blurs the editor and the kernel focus is the `ui:ai-panel.composer` scope.
- [x] Confirm existing `ai-panel.test.tsx` / `ai-prompt-composer.test.tsx` tests still pass.
- [x] Run `cd apps/kanban-app/ui && npx vitest run src/components/ai-panel.test.tsx src/components/ai-prompt-composer.test.tsx src/components/ai-panel.spatial.test.tsx src/components/ai-panel-container.test.tsx` — all green (45 passed).

## Review Findings (2026-05-19 16:05)

### Warnings
- [x] `apps/kanban-app/ui/src/components/ai-prompt-composer.tsx` — the bespoke `buildEscapeDrillOutExtension` hand-rolled a `Prec.highest` keymap Escape binding that could not preempt `@replit/codemirror-vim`'s insert-mode Escape. **Resolved:** that function is gone; the composer now routes Escape through the **shared** `buildSubmitCancelExtensions` helper, exactly like the filter editor, the markdown field, the command palette, and the field editors. The helper's vim path is the two-phase DOM-capture listener (`buildVimEscapeExtension`), so vim insert-mode Escape preempts the vim plugin correctly. No composer-specific keymap, no special case.

### Nits
- [x] `ComposerEditorDrillOutWiring` docstring "mirrors `FilterEditorDrillOutWiring`" wording. **Resolved:** the docstring now says "the handler body is the same shape as `FilterEditorDrillOutWiring`" and explicitly explains the one intentional simplification (single `useOptionalFullyQualifiedMoniker()` hook instead of the outer `FilterFormulaBarFocusable` layer-key guard + strict inner hook).

## Resolution of focus-system question (2026-05-19)

The user's directive ("figure how this was done elsewhere — this is a solved problem") chose between the two options I had surfaced: (A) route Escape through the shared `buildSubmitCancelExtensions` helper — the established codebase pattern for *every* CM6-editor drill-out in the app — vs (B) change `createKeyHandler`/`isEditableTarget` so per-scope `CommandDef` keys reach inside CM6 (a focus-system change with app-wide blast radius). Path (A) is the solved-elsewhere pattern. The current implementation is Path (A). No focus-system change.

## Workflow
- Use `/tdd`.
