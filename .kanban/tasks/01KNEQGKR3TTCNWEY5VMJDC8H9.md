---
assignees:
- claude-code
position_column: todo
position_ordinal: '8280'
title: File drop on attachment field broken — InspectorsContainer is outside FileDropProvider
---
## What\n\nDragging a file over the attachment field in the inspector shows no hover effect, and dropping does nothing.\n\n**Root cause:** `InspectorsContainer` is outside `BoardContainer` in App.tsx, so it's outside `FileDropProvider`.\n\n**Fix:** Move `InspectorsContainer` inside `BoardContainer`.\n\n**MANDATORY: TDD — write the failing test FIRST, then implement the fix.**\n\n**Files to modify:**\n- `kanban-app/ui/src/App.tsx` — move `<InspectorsContainer />` inside `<BoardContainer>`\n- `kanban-app/ui/src/components/inspectors-container.test.tsx` — add test: InspectorsContainer has access to FileDropProvider context (isDragging propagates)\n- `ARCHITECTURE.md` — verify container tree matches\n\n**MANDATORY: All dispatch via useDispatchCommand. Run tsc --noEmit before done.**\n\n## Acceptance Criteria\n- [ ] Test written FIRST proving FileDropProvider context reaches InspectorsContainer\n- [ ] Drag hover effect shows on attachment field in inspector\n- [ ] Drop triggers attachment add flow\n- [ ] ARCHITECTURE.md matches code\n\n## Tests\n- [ ] New test: InspectorsContainer receives isDragging from FileDropProvider (RED first, then GREEN)\n- [ ] `cd kanban-app/ui && pnpm vitest run` — all pass"