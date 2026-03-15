---
position_column: done
position_ordinal: ffff8c80
title: ensure_actor should update existing actor fields
---
**W1: `ensure` flag silently skips updates**

`state.rs` `ensure_os_actor` and MCP agent auto-create use `ensure: true`, which returns the existing actor without updating name/color/avatar. If the user changes their OS display name or profile picture, the stale actor persists forever.

**Fix:** When `ensure` finds an existing actor, compare and update changed fields (name, color, avatar) before returning.