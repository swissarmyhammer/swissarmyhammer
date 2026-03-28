---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffdc80
title: 'NIT: popClaim unregisters scope even if another claim uses the same moniker'
---
**File:** `kanban-app/ui/src/lib/entity-focus-context.tsx:149-154`\n\n**What:** `popClaim` does `registryRef.current.delete(entry.moniker)` unconditionally. If two FocusClaim components happen to claim the same moniker (e.g., the board moniker is claimed by both board-view's FocusClaim and a stale claim), popping one will delete the scope registration that the other still needs.\n\n**Why this matters:** Today this scenario is unlikely because monikers are unique per entity. But the code does not enforce uniqueness, and the board-view uses a fallback `boardMoniker` that is the same for all board cursors on column headers. If a mount/unmount race occurs, the scope registry entry could be deleted while a live claim still references that moniker, causing `getScope` to return null and breaking the scope chain sent to Rust.\n\n**Suggestion:** Before deleting from the registry, check whether any remaining claim in `claimsRef.current` uses the same moniker. Only delete if no other claim references it." #review-finding