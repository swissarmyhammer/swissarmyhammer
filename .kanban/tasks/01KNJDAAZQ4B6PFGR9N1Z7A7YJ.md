---
assignees:
- claude-code
depends_on:
- 01KNJD9KN51YAAY7QX5QY2M9SB
- 01KNJD7VB0QC38W3EETA84E15Y
position_column: doing
position_ordinal: '8280'
position_swimlane: null
title: 'FILTER-4: Update FilterEditor to use filter DSL language'
---
## What

Replace the JavaScript language mode in `FilterEditor` with the custom filter DSL language from FILTER-3. Update validation to parse the DSL instead of `new Function()`. Update placeholder text and help text.

### Files to modify
- `kanban-app/ui/src/components/filter-editor.tsx`:
  - Replace `import { javascript } from \"@codemirror/lang-javascript\"` with `import { filterLanguage } from \"@/lang-filter\"`
  - Replace `javascript()` in extensions with `filterLanguage()`
  - Replace `validateFilter()` — instead of `new Function()`, parse the expression using the Lezer parser and check for error nodes
  - Update placeholder from `Status !== \"Done\"` to `#bug && @will`
  - Update help text to describe DSL syntax: `#tag @user ^ref, &&/and, ||/or, !/not, ()`

### Validation approach
Use the Lezer parser to check for validity:
```ts
import { parser } from \"@/lang-filter/filter.grammar\";
function validateFilter(expr: string): string | null {
  const tree = parser.parse(expr);
  // Walk tree looking for error nodes
  let error: string | null = null;
  tree.iterate({ enter(node) { if (node.type.isError) error = \"Invalid expression\"; } });
  return error;
}
```

### Backend validation
The backend (FILTER-1) also validates. The frontend validation is for immediate UX feedback — the backend is the authoritative check.

## Acceptance Criteria
- [ ] Filter editor shows DSL syntax highlighting (colored tags, mentions, operators)
- [ ] Typing `#bug && @will` shows valid (no red border)
- [ ] Typing `#bug &&` shows error (incomplete expression)
- [ ] Placeholder text shows DSL example
- [ ] Enter saves, Escape cancels (existing behavior preserved)
- [ ] Vim/emacs keymaps still work
- [ ] `@codemirror/lang-javascript` is no longer imported in filter-editor.tsx

## Tests
- [ ] `kanban-app/ui/src/components/filter-editor.test.tsx` — update existing tests for new DSL syntax
- [ ] Test: valid DSL expression submits without error
- [ ] Test: invalid expression shows error message
- [ ] `npm test` in kanban-app passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.