---
title: 'TS2345: CodeMirror type incompatibility in editable-markdown.tsx line 128'
position:
  column: done
  ordinal: a2
---
In /Users/wballard/github/swissarmyhammer/swissarmyhammer-kanban/swissarmyhammer-kanban-app/ui/src/components/editable-markdown.tsx at line 128, the CodeMirror type is not assignable to CodeMirrorV. The state.vim property is typed as vimState | null | undefined but CodeMirrorV expects vimState (non-nullable). Need to add a null/undefined guard or update the type definition.