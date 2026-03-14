---
position_column: done
position_ordinal: fff880
title: Fix CM6 monospace font to inherit app font
---
CM6 editors default to monospace which is jarring against the app's system-ui font stack. Quick fix.

## Changes
- `ui/src/lib/cm-keymap.ts` — add `fontFamily: "inherit"` to `.cm-content` in `minimalTheme`

## Subtasks
- [ ] Add fontFamily inherit to minimalTheme .cm-content
- [ ] Verify all CM6 editors (EditableMarkdown, FieldPlaceholderEditor, date editor) use proportional font
- [ ] Run `npm test`