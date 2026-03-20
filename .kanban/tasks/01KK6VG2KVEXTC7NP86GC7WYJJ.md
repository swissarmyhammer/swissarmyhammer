---
position_column: done
position_ordinal: ffffee80
title: 'TS2345: command-palette.test.tsx(190,34) - mock getVimGlobal type mismatch with CodeMirror interface'
---
TypeScript error in src/components/command-palette.test.tsx at line 190: Argument of type '() => { state: { vim: {}; }; } | null' is not assignable to parameter of type '(view: EditorView) => CodeMirror | null'. The mock return type is missing 75+ properties from the CodeMirror interface. #test-failure