---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffac80
title: Add TextEditor component smoke tests to catch render crashes
---
## What

The TextEditor component had no component-level tests — only unit tests for `buildSubmitCancelExtensions` in isolation. A rebase silently dropped the `popup` prop, leaving a `ReferenceError` on an undefined variable that white-screened the entire app on any card click. No test caught it.

## Why

Unit tests for individual functions are necessary but not sufficient. A component smoke test that simply **renders** the component in its common configurations would have caught this ReferenceError immediately — the component throws during the `useCallback` evaluation.

## What to add

File: `kanban-app/ui/src/components/fields/text-editor.test.tsx`

### Smoke tests (must not throw)
- [ ] Renders TextEditor with minimal props (value, onCommit, onCancel)
- [ ] Renders TextEditor with onSubmit (compact/board card mode)
- [ ] Renders TextEditor with popup=true (quick-capture mode)
- [ ] Renders TextEditor with popup=false and onSubmit (the exact combo that crashed)

### Behavioral tests
- [ ] Editor mounts and receives focus (autoFocus)
- [ ] Changing value prop updates the editor content
- [ ] onCommit fires on blur

### Setup notes
- Will need `UIStateProvider` mock (for `useUIState` → `keymap_mode`)
- CodeMirror + vim extension in jsdom — the existing `cm-submit-cancel.test.ts` shows the pattern
- Keep tests fast — don't test vim key dispatch (that's already covered), just test that the component doesn't crash

## Acceptance Criteria
- [ ] All smoke tests pass for each prop combination
- [ ] A future rebase that drops a prop would fail the smoke tests
- [ ] `npx vitest run` — all tests pass