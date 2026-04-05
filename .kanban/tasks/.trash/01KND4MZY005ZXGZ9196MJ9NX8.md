---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: '8780'
title: 'VT-8: Exclude virtual tag slugs from CM6 autocomplete'
---
## What

Virtual tags should NOT appear in CM6 autocomplete when users type `#`. With Option B, virtual tags aren't in the entity store at all, so `search_mentions` naturally won't return them — **this may already work for free**.

However, we need to verify and add a guard. If someone manually types `#READY` in a task body, it would create a regular tag entity called READY (via auto-create on commit). We should prevent that.

**Files to check/modify:**
- `kanban-app/src/commands.rs` — `search_mentions` queries the entity store. Since virtual tags aren't entities, they won't appear. Verify this.
- `kanban-app/ui/src/components/fields/editors/multi-select-editor.tsx` — the editor auto-creates unknown tags on commit. If someone types `#READY`, it would create a real tag. Add a guard: reject slugs that match virtual tag registry slugs.
- Backend: `swissarmyhammer-kanban/src/task/tag.rs` — `TagTask` auto-creates tags. Guard against virtual tag slug names.

The virtual tag slug list needs to be available to the frontend for this guard. VT-3 (board metadata) provides this.

## Acceptance Criteria
- [ ] Typing `#` in CM6 editor does NOT show READY, BLOCKED, BLOCKING in autocomplete
- [ ] Manually typing `#READY` in body does NOT create a real tag entity named READY
- [ ] `tag task` with a virtual tag slug name returns error
- [ ] Regular tags still autocomplete and auto-create normally

## Tests
- [ ] Backend: `tag task` with slug "READY" errors when READY is a virtual tag
- [ ] Frontend: multi-select editor rejects virtual tag slugs on commit
- [ ] `cargo nextest run -p swissarmyhammer-kanban` and `pnpm --filter kanban-app-ui test` pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.

#virtual-tags