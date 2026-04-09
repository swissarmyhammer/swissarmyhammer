---
assignees:
- claude-code
depends_on:
- 01KND4JJV437GTZ7QFQ3JBM2F1
position_column: todo
position_ordinal: '8280'
title: 'VT-3: Expose virtual tag metadata + commands via API'
---
## What

The frontend needs virtual tag metadata (slug, color, description, commands) to render pills and wire context menus. Expose this through the `get board` response.

### Backend: include virtual_tags in board response

**Files to modify:**
- `swissarmyhammer-kanban/src/board/get.rs` — call `registry.metadata()` and include in board response as `virtual_tags` array:
  ```json
  {
    "virtual_tags": [
      {
        "slug": "READY",
        "color": "0e8a16",
        "description": "Task has no unmet dependencies",
        "commands": [
          { "id": "vtag.ready.start", "name": "Start Working", "context_menu": true }
        ]
      }
    ]
  }
  ```
- The board command handler needs access to the `VirtualTagRegistry`. Thread it through `KanbanContext` or pass as a parameter.

### Frontend: store and provide virtual tag metadata

**Files to modify:**
- `kanban-app/ui/src/lib/schema-context.tsx` (or a new `virtual-tag-context.tsx`) — parse `virtual_tags` from board response, provide via React context:
  ```ts
  interface VirtualTagMeta {
    slug: string;
    color: string;
    description: string;
    commands: Array<{ id: string; name: string; context_menu: boolean; keys?: KeyBindings }>;
  }
  // Map<slug, VirtualTagMeta>
  ```
- `kanban-app/ui/src/components/mention-pill.tsx` — consume virtual tag context for resolution fallback (VT-9 wires the visual distinction, VT-10 wires the commands)

### Frontend resolution order in MentionPill
1. Look up slug in entity store (existing — finds real tags)
2. If not found, look up slug in virtual tag metadata map → `isVirtual = true`
3. If not found, fall back to gray (existing)

## Acceptance Criteria
- [ ] `get board` response includes `virtual_tags` array with slug/color/description/commands
- [ ] Frontend context provides `Map<string, VirtualTagMeta>` from board response
- [ ] `MentionPill` resolves virtual tags via fallback to metadata map
- [ ] Virtual tag pills render with correct color and description tooltip
- [ ] Commands from metadata are available to downstream consumers (VT-10)

## Tests
- [ ] Backend: `get board` includes `virtual_tags` array with correct metadata and commands
- [ ] Frontend: virtual tag context provides metadata after board load
- [ ] Frontend: `MentionPill` renders virtual tag with correct color when not in entity store
- [ ] `cargo nextest run -p swissarmyhammer-kanban` and `pnpm --filter kanban-app-ui test` pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#virtual-tags