---
position_column: done
position_ordinal: d480
title: '[Low] No input length limit on search_entities query parameter'
---
The `search_entities` Tauri command (commands.rs line 352) accepts an arbitrary-length `query: String` with no upper bound check. While `dispatch_command` validates command IDs (max 128 chars, ASCII-only), `search_entities` passes the query directly to the fuzzy search engine.\n\nFor a desktop app this is low risk (the user controls input), but a very long query string could cause the fuzzy matcher to do excessive work. Consider adding a reasonable length cap (e.g. 500 chars) for consistency with the validation pattern used elsewhere.\n\nSeverity: Low (defense in depth)" #review-finding