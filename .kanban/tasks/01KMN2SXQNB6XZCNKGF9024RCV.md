---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffff9880
title: 'Fix failing tests: EntityCard - title rendering, edit mode, dispatch, context menu, progress bar (7 tests)'
---
File: /Users/wballard/github/swissarmyhammer-kanban/kanban-app/ui/src/components/entity-card.test.tsx\n\nFailing tests:\n- EntityCard > renders title as text via Field display\n- EntityCard > enters edit mode when title is clicked\n- EntityCard > saving edited title calls dispatch_command with correct params\n- EntityCard > entity.inspect command includes target moniker in context menu\n- EntityCard > progress bar > shows progress bar when progress field has items\n- EntityCard > progress bar > shows 0% progress when no items are completed\n- EntityCard > progress bar > shows 100% progress when all items are completed #test-failure