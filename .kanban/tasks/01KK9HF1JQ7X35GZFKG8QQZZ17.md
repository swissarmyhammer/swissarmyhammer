---
position_column: done
position_ordinal: ffffa680
title: 'False positive: "resolves typed text" test in multi-select-editor.test.tsx - getCmView returns null in jsdom'
---
The test at line 236 in ui/src/components/fields/editors/multi-select-editor.test.tsx wraps all assertions inside `if (view) { ... }` where `getCmView()` returns null in jsdom. The entire assertion block is skipped and the test passes vacuously with zero assertions. The `getCmView` helper looks for `cmEditor.cmView.view` on the DOM element, which is not populated by @uiw/react-codemirror in jsdom. The test needs to either: (1) find another way to dispatch text into CM6 in jsdom, or (2) use `expect(view).toBeTruthy()` and fix the underlying issue so the view is accessible.