---
assignees:
- claude-code
position_column: todo
position_ordinal: '9e80'
title: '[warning] TextEditor.saveInPlace changed from onCommit to onChange -- behavioral regression risk'
---
File: kanban-app/ui/src/components/fields/text-editor.tsx\n\nThe saveInPlace callback was changed from calling onCommitRef.current(text) to calling onChangeRef.current?.(text). Similarly, handleBlur was changed from commitAndExit (which called onCommit) to calling onChange.\n\nThis changes the semantic contract: previously, leaving vim insert mode or blurring the editor would COMMIT the value (triggering a save). Now it only fires onChange (which feeds a debounced save). If the user exits vim insert mode and immediately navigates away before the debounce fires, the save may be lost.\n\nSuggestion: Verify that the debounced save pipeline has a flush-on-unmount mechanism. If it does, document this in a comment. If not, this is a data-loss risk that should be addressed. #review-finding