---
assignees:
- claude-code
position_column: todo
position_ordinal: '8e80'
title: Display virtual tags (READY, BLOCKED, BLOCKING) on task cards and in MCP list output
---
## What

Virtual tags (READY, BLOCKED, BLOCKING) are computed correctly by the backend enrichment pipeline (`enrich_task_entity` in `swissarmyhammer-kanban/src/task_helpers.rs`) and stored in the `virtual_tags` field of enriched task JSON. However, **nothing displays them**:

1. **Frontend (Tauri app)**: Zero references to `virtual_tags` anywhere in `kanban-app/ui/src/`. Entity cards (`entity-card.tsx`) don't render any tags at all — neither user-assigned `tags` nor computed `virtual_tags`. The data arrives in the JSON payload but is ignored.

2. **MCP tool output**: The `list tasks` operation in the MCP server returns `tags` (user-assigned) but does not surface `virtual_tags` in its response shape. The enrichment runs (`enrich_all_task_entities` is called in `task/list.rs`), and `task_entity_to_rich_json` includes `virtual_tags` in the JSON, but the MCP response mapper strips it.

### Backend (working correctly — no changes needed)

- `swissarmyhammer-kanban/src/task_helpers.rs` — `enrich_task_entity()` evaluates the `VirtualTagRegistry` and sets `virtual_tags` field (e.g., `["READY"]`)
- `swissarmyhammer-kanban/src/task_helpers.rs` — `task_entity_to_rich_json()` includes `virtual_tags` in output JSON
- `swissarmyhammer-kanban/src/task/list.rs`, `task/get.rs`, `board/get.rs` — all call `enrich_all_task_entities` before building response

### Frontend fix needed

Add virtual tag pills to entity cards. Each virtual tag should render as a small colored badge using the colors from `VirtualTagMeta`:

| Tag | Color | Style |
|-----|-------|-------|
| READY | `#0e8a16` (green) | Solid badge |
| BLOCKED | `#e36209` (orange) | Solid badge |
| BLOCKING | `#d73a4a` (red) | Solid badge |

**Files to modify:**

1. **`kanban-app/ui/src/components/entity-card.tsx`** — Read `virtual_tags` from entity fields and render colored badge pills. The card already has a compact layout; pills should go below the title or in a footer row. Use the existing badge/pill pattern from `badge-list-display.tsx` or `mention-pill.tsx`.

2. **`kanban-app/ui/src/types/kanban.ts`** (or wherever the Task/Entity interface lives) — Ensure `virtual_tags: string[]` is part of the type definition so TypeScript is happy.

3. **MCP layer** — Check how `swissarmyhammer-kanban-mcp` (or the sah MCP server) maps task JSON to the `list tasks` response. If there's a response mapper that picks specific fields, add `virtual_tags` to it.

### Note on `compute-virtual-tags` derive stub

In `swissarmyhammer-kanban/src/defaults.rs`, the `compute-virtual-tags` derive function is a **stub returning empty array** (comment: "Populated by the enrichment pipeline in a later card"). This is for the entity schema computed field system, NOT the command-level enrichment. The command-level enrichment (which works) bypasses this derive. The stub may need to be wired up if any path relies on the schema-level computation, but for this card the focus is on displaying what's already computed.

## Acceptance Criteria

- [ ] Task cards in the Tauri app show virtual tag pills (READY in green, BLOCKED in orange, BLOCKING in red) when applicable
- [ ] Tasks with no dependencies in a non-terminal column show a READY pill
- [ ] Tasks with unmet dependencies show a BLOCKED pill
- [ ] Tasks that other tasks depend on show a BLOCKING pill
- [ ] Virtual tags are visually distinct from user-assigned tags (different style or position)
- [ ] MCP `list tasks` output includes `virtual_tags` array in each task
- [ ] `cargo test` and `pnpm test` pass

## Tests

- [ ] `kanban-app/ui/src/components/entity-card.test.tsx` — render a card with `virtual_tags: ["READY"]` in entity fields, assert a green READY badge is in the DOM
- [ ] `kanban-app/ui/src/components/entity-card.test.tsx` — render a card with `virtual_tags: ["BLOCKED", "BLOCKING"]`, assert both badges appear
- [ ] `kanban-app/ui/src/components/entity-card.test.tsx` — render a card with `virtual_tags: []`, assert no virtual tag badges
- [ ] `cargo test -p swissarmyhammer-kanban` — existing enrichment tests still pass
- [ ] `pnpm test` — all frontend tests pass

## Workflow

- Use `/tdd` — write failing tests first, then implement to make them pass.