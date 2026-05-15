---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffcd80
title: 'Fix entity-card.test.tsx failures (7 tests): fields not rendering in card'
---
EntityCard renders an empty `flex-1 min-w-0 break-words space-y-0.5` div — no field content is being rendered. Tests expect title text, tag pills, progress bar and context menu items but the card body is empty.\n\nFailing tests:\n- renders title as text via Field display (getByText \"Hello **world**\" → not found)\n- enters edit mode when title is clicked (same — no title rendered)\n- saving edited title calls dispatch_command with correct params (no title rendered)\n- entity.inspect command includes target moniker in context menu (show_context_menu call not found)\n- progress bar > shows progress bar when progress field has items (no [role=progressbar])\n- progress bar > shows 0% progress when no items are completed\n- progress bar > shows 100% progress when all items are completed\n\nFile: `/Users/wballard/github/swissarmyhammer-kanban/kanban-app/ui/src/components/entity-card.test.tsx`"