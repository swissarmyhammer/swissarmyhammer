---
assignees:
- claude-code
depends_on:
- 01KNHP391SXAQ5H2YXEK2MYJD1
position_column: done
position_ordinal: ffffffffffffffffffffffffffffa380
title: 'WARNING: avatar.tsx constructs moniker from actorId instead of entity.moniker'
---
**File:** `kanban-app/ui/src/components/avatar.tsx` — scopeMoniker\n\n**What:** The FocusScope moniker is computed as:\n```ts\nconst scopeMoniker = moniker(\"actor\", actorId);\n```\nThe actor entity IS resolved from the store via `getEntity(\"actor\", actorId)`, so `actor.moniker` is available when found.\n\n**Why:** Same pattern as mention-pill — works for current simple monikers but doesn't use the backend-provided value.\n\n**Suggestion:** Use `actor?.moniker ?? moniker(\"actor\", actorId)`.\n\n**Verification:** Right-click an avatar, confirm the FocusScope moniker matches the backend. #review-finding