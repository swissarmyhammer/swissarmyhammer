---
assignees:
- claude-code
depends_on:
- 01KN71ND180SYB0DA40SQ9YCDZ
position_column: done
position_ordinal: fffffffffffffffffffff880
title: Rewrite flush_and_emit_for_handle to use StoreContext bridge
---
Replace watcher-based flush with store_context.flush_all() + enrichment bridge in commands.rs