---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffe980
title: Quick capture popup should auto-grow instead of scrolling
---
quick-capture.tsx:198\n\nThe popup has `overflow-hidden` on the container. When the user types multiple lines, the CM6 editor scrolls internally instead of growing the popup. The user explicitly flagged this as unacceptable UI.\n\nSuggestion: Remove fixed height constraint, let the editor grow vertically (up to a max), and resize the Tauri window to match content height.