---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffff8380
title: 'VT-00: Remove per-tag task_count from get board response'
---
## What

The `get board` response currently includes a `task_count` per tag, computed by scanning all task bodies for `#tag` occurrences. This is expensive (O(tasks × tags)), inaccurate for virtual tags, and not used by the frontend in any meaningful way. Remove it before adding virtual tags to avoid the count problem entirely.

**Files to modify:**
- `swissarmyhammer-kanban/src/board/get.rs` (or wherever `get board` assembles the tag list) — stop computing per-tag task counts, return tags without `task_count`

**Files to check for consumers:**
- `kanban-app/ui/src/` — verify no frontend component reads `tag.task_count` from the board response
- `builtin/_partials/tool-use/kanban.md` — update MCP tool documentation if it mentions tag counts
- `.agents/*/AGENT.md` — check if any agent instructions reference tag counts

## Acceptance Criteria
- [ ] `get board` response no longer includes `task_count` per tag
- [ ] No frontend regression (nothing relied on tag counts)
- [ ] Board loading is faster (no body scanning for tag counts)
- [ ] MCP tool docs updated if they referenced tag counts

## Tests
- [ ] Update existing `get board` tests that assert on tag structure
- [ ] `cargo nextest run -p swissarmyhammer-kanban` passes
- [ ] `pnpm --filter kanban-app-ui test` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#virtual-tags