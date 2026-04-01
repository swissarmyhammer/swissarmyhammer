---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffb380
title: PerspectiveContext name_index corrupted when adding duplicate names
---
context.rs:76-80\n\nWhen `write` adds a new perspective (not an update by ID), it inserts the name into `name_index`. If a perspective with the same name already exists (added via a different ID), the old index entry is overwritten but the old perspective remains in the `perspectives` vec. The old perspective's name is now orphaned -- it cannot be found by name but still appears in `all()`.\n\nThis is a data integrity issue in the index. The `write` method for new entries should check `name_index` and either:\n1. Return an error for duplicate names, or\n2. Remove the old name entry from the vec (and fix indexes), or\n3. Explicitly document that duplicate names are allowed and `get_by_name` returns the last-written.\n\nThis is closely related to the AddPerspective uniqueness finding but is specifically about the storage layer contract.\n\nVerification: Write a unit test that adds two perspectives with different IDs but the same name, then verifies `all().len()` and `get_by_name` behavior." #review-finding