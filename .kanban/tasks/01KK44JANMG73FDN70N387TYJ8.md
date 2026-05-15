---
position_column: done
position_ordinal: ffffde80
title: 'markdown-display: forwardRef import after component definition'
---
In markdown-display.tsx, `forwardRef` is imported on line 55, after the MarkdownDisplay function definition. While this compiles (hoisting), it breaks the convention of all imports at the top of the file and will confuse linters/readers. Move to the top import block.