---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffff8e80
title: '[nit] enrich_computed_fields has excessive tracing::info for normal operation'
---
File: kanban-app/src/commands.rs (enrich_computed_fields)\n\nThe function now logs at info level for every computed field appended and a summary for every event processed. In a board with many tasks and computed fields, this will produce high-volume logs at INFO level during normal file watcher operations.\n\nSuggestion: Downgrade the per-field \"appending computed field\" log from tracing::info to tracing::debug. Keep the summary log at debug as well. The warn for missing computed fields is appropriate. #review-finding