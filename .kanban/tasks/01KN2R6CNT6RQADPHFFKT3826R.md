---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffad80
title: Install @tanstack/react-virtual dependency to fix board-view and column-view tests
---
**Files:** kanban-app/ui/src/components/board-view.test.tsx, kanban-app/ui/src/components/column-view.test.tsx, kanban-app/ui/src/components/column-view.tsx\n\n**Error:**\n```\nError: Failed to resolve import \"@tanstack/react-virtual\" from \"src/components/column-view.tsx\". Does the file exist?\n```\n\n**Also causes TypeScript errors:**\n- TS2307: Cannot find module '@tanstack/react-virtual'\n- TS7006: Parameter 'index' implicitly has an 'any' type (line 644)\n- TS7006: Parameter 'virtualRow' implicitly has an 'any' type (line 658)\n\n**Fix:** Run `pnpm add @tanstack/react-virtual` in kanban-app/ui, then verify tests pass and tsc --noEmit is clean.\n\n**Pre-existing:** This failure existed before this session. The dependency is used in column-view.tsx but was never added to package.json." #test-failure