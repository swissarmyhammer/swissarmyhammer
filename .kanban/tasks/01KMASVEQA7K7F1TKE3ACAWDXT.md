---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffff380
title: Define EditorProps contract — editors own save, containers own lifecycle
---
## What

Refactor the `EditorProps` interface in `kanban-app/ui/src/components/fields/editors/markdown-editor.tsx` so editors receive entity identity and save themselves.

### Files to modify
- `kanban-app/ui/src/components/fields/editors/markdown-editor.tsx` — `EditorProps` interface

### Approach
1. Add `entityType: string`, `entityId: string`, `fieldName: string` to `EditorProps`
2. Replace `onCommit: (value: unknown) => void` with `onDone: () => void` — lifecycle signal only
3. Keep `onCancel: () => void` — means "discard and close"
4. Keep `onSubmit` optional — container hint for "Enter means close" (grid)
5. Keep `mode: "compact" | "full"`

The editor contract becomes: I receive entity identity, I call `useFieldUpdate().updateField()` to save, I call `onDone()` when I'm finished.

## Acceptance Criteria
- [ ] `EditorProps` has `entityType`, `entityId`, `fieldName`
- [ ] `onCommit` replaced with `onDone` (no value parameter)
- [ ] TypeScript compilation succeeds (editors will have type errors until updated — that's expected)

## Tests
- [ ] No test changes in this card — downstream editor cards add tests
- [ ] `npx tsc --noEmit` may show errors in editors — that's the signal for the next cards