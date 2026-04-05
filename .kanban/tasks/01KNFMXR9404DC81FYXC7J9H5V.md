---
assignees:
- claude-code
position_column: todo
position_ordinal: ab80
title: TauriClipboardProvider error matching uses fragile string comparison
---
kanban-app/src/state.rs -- TauriClipboardProvider::read_text()\n\nThe `read_text()` implementation matches error messages with `msg.contains(\"empty\") || msg.contains(\"format\")` to distinguish \"clipboard empty\" from real failures. The code itself calls this out as a HACK with a comment noting it is fragile.\n\nThis could silently swallow real errors if the Tauri plugin changes its error messages, or falsely trigger on unrelated error messages that happen to contain these substrings.\n\nSuggestion: File an issue upstream on tauri-plugin-clipboard-manager for structured error variants. In the meantime, add a log warning when suppressing an error so it is observable. #review-finding