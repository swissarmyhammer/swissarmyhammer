---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffff8480
project: ai-panel
title: 'AI panel: composer focus scope nests the model picker under the prompt'
---
## What

Jumping to the AI composer ("Ask the AI agent…") puts DOM focus into the CM6 prompt, but pressing Enter activates the **model picker** instead of editing the prompt. The spatial-nav focus scope is nested wrong.

Current structure (after the PromptInput rework):

- `ComposerArea` (`apps/kanban-app/ui/src/components/ai-panel.tsx`) wraps the entire composer in `<AiPanelFocusScope moniker="ui:ai-panel.composer">`.
- Inside, `AiPromptComposer` (`apps/kanban-app/ui/src/components/ai-prompt-composer.tsx`) is one bordered container holding the CM6 editor body **and** the footer toolbar.
- The footer's `ComposerModelSelect` registers its own focus leaf — `ui:ai-panel.model-selector` (via `AiPanelPressable`).
- The CM6 editor body has **no** focus scope of its own.

The CM6 prompt and the model picker are **two independent controls** and must each be their own spatial-nav target — siblings, not one nested in the other. This mirrors the filter formula bar: a CM6 editor is its own scope you land on and drill into.

## Approach

Remove the scope on the surrounding composer container; give the CM6 editor and the model picker each their own scope/leaf as siblings under the `ui:ai-panel` zone. The CM6 scope must actually drive the cursor into the editor on drill-in (see Review Findings).

Do not change the Enter-submit / Shift-Enter keymap, the submit/stop control, or `ai.focus`.

## Acceptance Criteria
- [x] The surrounding bordered composer container is not a focus scope; only the CM6 body is `ui:ai-panel.composer` and the picker is `ui:ai-panel.model-selector`.
- [x] `ui:ai-panel.composer` and `ui:ai-panel.model-selector` are siblings under the `ui:ai-panel` zone — neither nested inside the other.
- [x] Drilling into `ui:ai-panel.composer` (Enter) actually moves the editing cursor into the CM6 editor — see Review Findings.
- [x] The model picker is independently reachable as its own spatial-nav leaf.
- [x] `ai.focus`, the Enter/Shift-Enter keymap, and the submit/stop control are unchanged.

## Workflow
- Use `/tdd`.

## Review Findings (2026-05-19 20:02)

### Blockers
- [x] `apps/kanban-app/ui/src/components/ai-prompt-composer.tsx` — drilling into the composer does not actually focus the CM6 editor. A bare `<FocusScope>` (via `AiPanelFocusScope`) only *registers* the scope as a nav target; landing on it and pressing Enter does NOT move the editing cursor into the editor. The established pattern is `FilterFormulaBarFocusable` in `apps/kanban-app/ui/src/components/perspective-tab-bar.tsx`: the `<FocusScope>` is given a `commands` prop holding a per-scope drill-in `CommandDef` (`id: "...drillIn"`, `keys: { cua/vim/emacs: "Enter" }`) whose `execute` calls `editorRef.current?.focus()` — that command is what actually drives the cursor in. The composer's CM6 scope has no such command, so drill-in is a no-op. Fix: (1) extend `AiPanelFocusScope` (`ai-panel-focus.tsx`) to accept and forward a `commands` prop to the underlying `FocusScope` — it currently does not; (2) in `AiPromptComposer`, pass the CM6 body's `AiPanelFocusScope` a `commands` array with a drill-in `CommandDef` keyed to Enter whose `execute` calls `editorRef.current?.focus()`. RESOLVED: `AiPanelFocusScope` now accepts a `commands?: readonly CommandDef[]` prop and forwards it to `<FocusScope commands={...}>`. `AiPromptComposer` builds a `ui.ai-panel.composer.drillIn` `CommandDef` (`keys: { cua/vim/emacs: "Enter" }`, `execute` calls `editorRef.current?.focus()`) and passes it to the CM6 body's `AiPanelFocusScope`.
- [x] Use the shared `TextEditor` primitive's handle for the focus call — `editorRef` is already a `TextEditorHandle` (`@/components/fields/text-editor`), and `TextEditorHandle.focus()` calls the underlying `view.focus()`. Do NOT reach into a raw CodeMirror `EditorView` directly. (The composer already renders `<TextEditor>`, not hand-rolled CM6 — keep it that way.) RESOLVED: the drill-in command's `execute` calls `editorRef.current?.focus()` — the existing `TextEditorHandle`. No raw `EditorView` access added; the composer still renders `<TextEditor>`.

### Tests
- [x] Add/extend an `ai-panel.spatial.test.tsx` test that drives the drill-in (Enter on the focused `ui:ai-panel.composer` scope) and asserts the CM6 prompt (`[role="textbox"][aria-label="Message the AI agent"]`) actually receives DOM focus — not just that the scope is registered. DONE: added "Enter on the focused composer leaf drives DOM focus into the CM6 prompt" to `ai-panel.spatial.test.tsx` — mounts `AiPanel` inside `<AppShell>` so `KeybindingHandler` is live, seeds spatial focus on the composer leaf, fires a real `keyDown(Enter)`, and asserts `document.activeElement` is the CM6 prompt. Verified TDD-RED: the test fails when the `commands` prop is removed.
- [x] Run `cd apps/kanban-app/ui && npx vitest run src/components/ai-panel.test.tsx src/components/ai-prompt-composer.test.tsx src/components/ai-panel.spatial.test.tsx src/components/ai-panel-container.test.tsx` — all green. DONE: 4 files / 43 tests pass; `npx tsc --noEmit` clean.
