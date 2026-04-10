---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffbc80
project: null
title: Implement file watcher for incremental indexing
---
Currently FileEvent enum exists but no actual file watching is implemented. Need to detect file changes and trigger incremental re-indexing instead of full rebuilds.