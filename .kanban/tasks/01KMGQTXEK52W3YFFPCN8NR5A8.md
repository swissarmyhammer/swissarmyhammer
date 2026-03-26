---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffbb80
title: Fix missing js-yaml dependency in editor-save.test.tsx
---
The test file `src/components/fields/editors/editor-save.test.tsx` fails to load because `js-yaml` cannot be resolved as an import.\n\nError:\n```\nFailed to resolve import \"js-yaml\" from \"src/components/fields/editors/editor-save.test.tsx\". Does the file exist?\n```\n\nThe package `js-yaml` is imported in the test file but is not installed in `/Users/wballard/github/swissarmyhammer-kanban/kanban-app/ui`. Either add it to package.json as a dependency/devDependency, or remove the import if it is unnecessary. #test-failure