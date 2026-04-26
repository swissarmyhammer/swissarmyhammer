---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffff8380
title: SavePerspectiveCmd always creates, never upserts by name despite doc comment claiming otherwise
---
**Severity**: Medium (correctness / doc mismatch)\n**Layer**: Correctness\n**Files**: `swissarmyhammer-kanban/src/commands/perspective_commands.rs:40-71`\n\nThe doc comment on `SavePerspectiveCmd` says: \"If a perspective with the given name already exists, it is updated. Otherwise a new perspective is created.\"\n\nBut the implementation at lines 66-71 always constructs a new `AddPerspective` (which generates a new ULID). It never checks for an existing perspective by name to update it. Since duplicate names are now allowed, calling save twice with the same name creates two separate perspectives.\n\nEither the doc comment is stale (from before duplicate names were allowed) and should be updated, or the implementation needs to do upsert-by-name logic.\n\n**Fix**: Update the doc comment to match the actual behavior: \"Creates a new perspective with the given name. Multiple perspectives may share the same name.\"" #review-finding