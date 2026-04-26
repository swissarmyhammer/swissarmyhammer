---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffc680
title: '[nit] BoardView groupValue prop uses undefined check instead of explicit presence'
---
File: kanban-app/ui/src/components/board-view.tsx (BoardView baseLayout sort)\n\nThe grouping sort logic uses `groupValue === undefined` to decide whether to cluster by group. This is correct but fragile -- if groupValue is ever passed as null instead of undefined, the behavior silently changes. The prop is typed as `string | undefined` so this is technically fine but a comment explaining the sentinel semantics would help.\n\nSuggestion: Add a brief comment: \"groupValue is undefined when no grouping is active, present (even empty string) when inside a GroupSection.\" #review-finding