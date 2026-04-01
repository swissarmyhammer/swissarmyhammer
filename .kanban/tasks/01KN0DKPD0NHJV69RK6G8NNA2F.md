---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffff8c80
title: 'TS error: quick-capture.tsx - Promise<unknown> not assignable to () => void | Promise<void> (TS2322)'
---
File: kanban-app/ui/src/components/quick-capture.tsx lines 143 and 158. Async callbacks return Promise<unknown> but expected () => void | Promise<void>. #test-failure