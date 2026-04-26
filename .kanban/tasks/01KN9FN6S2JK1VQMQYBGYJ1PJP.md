---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffe80
title: 'Pattern divergence: PerspectiveContext.delete() returns Perspective vs EntityContext.delete() returns Option<UndoEntryId>'
---
**Severity**: Low (pattern consistency)\n**Layer**: Design / Pattern following\n**Files**: `swissarmyhammer-perspectives/src/context.rs:124`, `swissarmyhammer-entity/src/context.rs:243`\n\n`EntityContext::delete()` returns `Result<Option<UndoEntryId>>` -- it doesn't return the deleted entity because the entity may not even be loaded (it's ID-based). It reads only for attachment cleanup.\n\n`PerspectiveContext::delete()` returns `Result<Perspective>` -- the deleted object. This is arguably more useful for callers (the delete command uses it for the response JSON), but it diverges from entity's pattern.\n\nThis is a reasonable difference given that PerspectiveContext holds items in memory, so returning the deleted value is free. Not recommending a change -- just noting the divergence for awareness. If future alignment is desired, EntityContext could gain a `delete_and_return()` method." #review-finding