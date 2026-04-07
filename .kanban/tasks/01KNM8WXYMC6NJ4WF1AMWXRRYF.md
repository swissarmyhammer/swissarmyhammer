---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffff9280
title: '[warning] GroupSection uses hardcoded maxHeight style instead of flex layout'
---
File: kanban-app/ui/src/components/group-section.tsx\n\nThe expanded content uses `style={{ maxHeight: \"calc(100vh - 6rem)\" }}` which is a magic number. This will not adapt to different toolbar heights, window chrome, or nested layouts. If the toolbar or nav changes height, this calc breaks silently.\n\nSuggestion: Use flex-based constraints (`flex-1 min-h-0 overflow-auto`) to let the layout engine handle sizing, consistent with how BoardView and other containers manage their height. #review-finding