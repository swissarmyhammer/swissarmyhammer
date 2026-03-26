---
assignees:
- claude-code
position_column: todo
position_ordinal: d180
title: 'Fix inspector-focus-bridge.test.tsx failures (3 tests): entity-inspector not rendering'
---
InspectorFocusBridge tests fail because EntityInspector is not rendering (stuck on loading schema), so [data-testid=\"entity-inspector\"] is not found and field rows are absent.\n\nFailing tests:\n- renders EntityInspector inside a command scope (entity-inspector testid not found)\n- first field is focused on mount (field-row-title not present, hasAttribute returns undefined)\n- renders all navigable fields (field-row-title not found)\n\nDepends on the SchemaContext null-types fix also needed by entity-inspector tests.\n\nFile: `/Users/wballard/github/swissarmyhammer-kanban/kanban-app/ui/src/components/inspector-focus-bridge.test.tsx`"