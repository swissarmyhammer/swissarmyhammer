---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffbc80
title: BoardSelector tear-off button uses raw &lt;button&gt; instead of shadcn Button component
---
kanban-app/ui/src/components/board-selector.tsx:96-106\n\nThe tear-off button is implemented as a raw `<button>` element with inline Tailwind classes:\n\n```tsx\n<button\n  type=\"button\"\n  className=\"p-1 rounded text-muted-foreground/40 hover:text-muted-foreground hover:bg-muted transition-colors\"\n  ...\n>\n```\n\nThe project memory explicitly states: \"never use raw HTML when shadcn exists\". The codebase uses shadcn's `Button` component with `variant=\"ghost\"` and `size=\"icon\"` throughout the nav bar area (e.g. the Info and Search buttons in nav-bar.tsx follow the same raw pattern, but both were pre-existing). This new button should use `<Button variant=\"ghost\" size=\"icon\">` for consistency.\n\nSuggestion: Replace with `<Button variant=\"ghost\" size=\"icon\" title=\"Open in new window\">` from `@/components/ui/button`." #review-finding