---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffff9a80
title: Fix missing js-yaml dependency in editor-save.test.tsx
---
The test file `src/components/fields/editors/editor-save.test.tsx` fails to run because `js-yaml` is not installed as a dependency.\n\nError: `Failed to resolve import \"js-yaml\" from \"src/components/fields/editors/editor-save.test.tsx\". Does the file exist?`\n\nFix: run `npm install js-yaml` (or `npm install --save-dev js-yaml`) in `/Users/wballard/github/swissarmyhammer-kanban/kanban-app/ui`. #test-failure