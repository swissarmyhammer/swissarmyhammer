---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffdc80
title: Add test for CM6 undo guard in executeCommand
---
kanban-app/ui/src/components/app-shell.tsx:57-62\n\nThe CM6 guard checks `document.activeElement?.closest('.cm-editor')` and returns false for app.undo/app.redo when inside a CodeMirror editor. No test verifies this guard.\n\nTest in app-shell.test.tsx:\n1. When activeElement is inside .cm-editor, executeCommand('app.undo') returns false (no dispatch)\n2. When activeElement is NOT inside .cm-editor, executeCommand('app.undo') dispatches normally\n3. Other commands (e.g. 'app.quit') still dispatch when inside .cm-editor\n\nRequires creating a DOM element with class .cm-editor and focusing it. #coverage-gap