---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff8480
title: Update EntityContext archive/unarchive to use StoreHandle
---
Update EntityContext::archive() and unarchive() to delegate to StoreHandle and push undo entries, matching the pattern used by write() and delete()