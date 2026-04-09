---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8980'
title: 'VT-10: Wire virtual tag commands into MentionPill context menu'
---
## What

Virtual tag pills need their own context menu commands — not entity commands (they're not entities) but strategy-declared commands from the registry. This follows the same pattern as view commands: declarative definitions from the backend, execute handlers wired on the frontend.

### Frontend command wiring

When `MentionPill` resolves a tag from virtual metadata (`isVirtual = true` from VT-9), it:
1. Does NOT call `useEntityCommands` (no entity, no entity commands)
2. Does NOT add `task.untag` extra command (virtual tags can't be removed)
3. Reads `commands` from the `VirtualTagMeta` (provided by VT-3 context)
4. Builds `CommandDef[]` from those commands, with execute handlers that dispatch to the backend via `backendDispatch`
5. Passes these to `FocusScope`

The execute handler for each virtual tag command dispatches to the backend like any entity command — `backendDispatch({ cmd: command.id, target: taskMoniker, ... })`. The backend command handler implements the actual logic (e.g. "Start Working" moves the task).

**Files to modify:**
- `kanban-app/ui/src/components/mention-pill.tsx`:
  - When `isVirtual`: build `CommandDef[]` from `VirtualTagMeta.commands` instead of entity commands
  - Wire execute handlers via `backendDispatch` targeting the **task** (not the tag — virtual tags aren't entities)
  - The `taskId` prop (already on MentionPill) provides the target

**Note**: The actual backend command handlers for virtual tag actions (e.g. `vtag.ready.start`) are implemented in the individual strategy cards (VT-5/6/7). This card only wires the frontend dispatch.

## Acceptance Criteria
- [ ] Right-click on virtual tag pill shows strategy-declared commands
- [ ] Commands dispatch to backend with task as target
- [ ] No entity commands (cut/copy/paste/archive) shown
- [ ] No `task.untag` shown
- [ ] Regular tag pills still show full entity context menu (no regression)

## Tests
- [ ] `kanban-app/ui/src/components/mention-pill.test.tsx` — test that virtual tag pill shows strategy commands
- [ ] `kanban-app/ui/src/components/mention-pill.test.tsx` — test that virtual tag pill has no entity commands
- [ ] `kanban-app/ui/src/components/mention-pill.test.tsx` — test that regular tag pill still shows entity commands
- [ ] `pnpm --filter kanban-app-ui test` passes

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#virtual-tags