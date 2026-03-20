---
assignees:
- assistant
position_column: done
position_ordinal: ffb980
title: 'Card 12: Frontend-driven menu generation'
---
New menu-sync.ts module collects commands with menuPlacement, serializes as JSON manifest, sends to Rust via rebuild_menu_from_manifest Tauri command. Rust builds native menu from manifest.