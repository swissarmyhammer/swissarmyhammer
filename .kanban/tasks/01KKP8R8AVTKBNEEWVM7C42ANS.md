---
position_column: done
position_ordinal: cf80
title: '[Low] Frontend search does client-side fuzzy match instead of using search_entities IPC'
---
The `search_entities` Tauri command was added in commands.rs and registered in main.rs, but the command-palette.tsx search mode does NOT call it. Instead, the frontend collects all entities from the EntityStore and runs its own `fuzzyMatch` client-side (lines 140-154 of command-palette.tsx).\n\nThis is a design choice — the backend search_entities command exists but is unused by the frontend. This is fine if the intent is to use the backend for future MCP/CLI search and the frontend for UI search (where the EntityStore already has all data). However, having dead IPC surface area is a minor concern.\n\nConsider: either remove `search_entities` if it is not yet needed, or document it as reserved for CLI/MCP use. Currently it is dead code from the UI perspective.\n\nSeverity: Low (dead code / design intent question)" #review-finding