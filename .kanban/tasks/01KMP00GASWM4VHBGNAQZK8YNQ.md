---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffe080
title: 'WARNING: console.warn debug logging left in focus change hot path'
---
**File:** `kanban-app/ui/src/lib/entity-focus-context.tsx:62`\n\n**What:** `console.warn('[FocusScope] focus -> ...')` fires on every focus change -- every cursor movement in board nav, every inspector open/close, every click on an entity. This is a debug log in a production code path.\n\n**Why this matters:** Per project convention (feedback_frontend_logging.md), `console.warn` is used for instrumentation. But this is extremely chatty -- board navigation alone can trigger dozens of focus changes per second during keyboard repeat. It will pollute the unified log and slow down profiling.\n\n**Suggestion:** Either remove the log entirely (the Rust side already receives `ui.setFocus` and can log there), or gate it behind a debug flag / reduce to `console.debug` level so it does not appear in normal `log show` output." #review-finding