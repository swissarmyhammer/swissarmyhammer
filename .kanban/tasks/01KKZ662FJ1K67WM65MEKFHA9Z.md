---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffac80
title: 'Nit: TS interfaces mark board_path as required but filter treats it as optional'
---
kanban-app/ui/src/App.tsx:62-84\n\nThe `EntityCreatedEvent`, `EntityRemovedEvent`, and `EntityFieldChangedEvent` interfaces declare `board_path: string` (required), but the runtime filter uses `if (board_path && ...)` which implies it could be missing/falsy. Since all events now always include `board_path` via `BoardWatchEvent`, the required type is technically correct. However, marking it `board_path?: string` would be safer during development/testing if you ever emit a raw `WatchEvent` without the wrapper.\n\nNot a correctness issue — just a type-level inconsistency with the defensive runtime check."