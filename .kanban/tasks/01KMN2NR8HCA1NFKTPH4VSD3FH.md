---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffc780
title: 'Fix editable-markdown.test.tsx failure (1 test): tag pill not rendered for #bug mention'
---
EditableMarkdown displays "Fix the #bug in login" as plain text rather than rendering #bug as a TagPill component in display mode.\n\nFailing test:\n- multiline editing with mention types > renders tag pills in display mode with mentions loaded (getByText('#bug') not found — text is plain '#bug' inside a paragraph)\n\nFile: `/Users/wballard/github/swissarmyhammer-kanban/kanban-app/ui/src/components/editable-markdown.test.tsx`"