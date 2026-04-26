---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffff8180
title: 'vitest: dispatchCommand tests fail — getCurrentWindow().label throws in test environment'
---
2 test files, 3 tests failing in kanban-app/ui with TypeError: Cannot read properties of undefined (reading 'metadata') from getCurrentWindow() in src/lib/command-scope.tsx:217.\n\nFailing tests:\n1. src/lib/command-scope.test.tsx > dispatchCommand > dispatches to Rust by id when no execute is set\n2. src/lib/command-scope.test.tsx > dispatchCommand > dispatches to Rust when no execute is set (no args)\n3. src/lib/context-menu.test.tsx > dispatchContextMenuCommand > dispatches to Rust by id when no execute is set\n\nRoot cause: dispatchCommand calls getCurrentWindow().label (src/lib/command-scope.tsx line 217) but @tauri-apps/api/window.js getCurrentWindow() returns undefined in the vitest jsdom environment because Tauri window metadata is not injected.\n\nFiles:\n- /Users/wballard/github/swissarmyhammer-kanban/kanban-app/ui/src/lib/command-scope.tsx\n- /Users/wballard/github/swissarmyhammer-kanban/kanban-app/ui/src/lib/command-scope.test.tsx\n- /Users/wballard/github/swissarmyhammer-kanban/kanban-app/ui/src/lib/context-menu.test.tsx #test-failure