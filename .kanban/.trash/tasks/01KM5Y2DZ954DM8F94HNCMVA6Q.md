---
assignees:
- claude-code
position_column: todo
position_ordinal: '8980'
title: 'vitest: dispatchCommand calls getCurrentWindow() without mock — TypeError in test environment'
---
3 vitest tests fail because `dispatchCommand` calls `getCurrentWindow().label` (line 217 of src/lib/command-scope.tsx) and the Tauri `@tauri-apps/api/window` module is not mocked in the test environment, so `getCurrentWindow()` returns undefined.

Failing tests:
- src/lib/command-scope.test.tsx > dispatchCommand > dispatches to Rust by id when no execute is set
- src/lib/command-scope.test.tsx > dispatchCommand > dispatches to Rust when no execute is set (no args)
- src/lib/context-menu.test.tsx > dispatchContextMenuCommand > dispatches to Rust by id when no execute is set

Error: TypeError: Cannot read properties of undefined (reading 'metadata')
  at getCurrentWindow node_modules/@tauri-apps/api/window.js:85:50
  at Module.dispatchCommand src/lib/command-scope.tsx:217:20

Fix: mock `getCurrentWindow` from `@tauri-apps/api/window` in the relevant test files (or in the vitest setup) to return an object with a `label` property. #test-failure