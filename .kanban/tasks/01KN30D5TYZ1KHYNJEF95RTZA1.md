---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffb280
title: Perspective name uniqueness not enforced on AddPerspective
---
add.rs:81-108 and context.rs:63-84\n\nThe `AddPerspective` command and `PerspectiveContext::write` allow creating multiple perspectives with the same name. The `name_index` HashMap in `PerspectiveContext` maps name->index, so a second perspective with the same name silently overwrites the first in the name index. The first perspective becomes unreachable by name (orphaned in the index) but still exists in the `perspectives` vec.\n\nThis means:\n- `get_by_name` returns only the most recently written perspective with that name\n- `all()` returns both, potentially confusing the UI\n- `SavePerspectiveCmd` relies on `get_by_name` to detect duplicates, so it works for the command path, but direct `AddPerspective` callers can create duplicates\n\nSuggestion: Either enforce uniqueness in `AddPerspective::execute` (return an error if a perspective with the same name exists) or document that names are advisory and not unique. If enforcing, add a `name_exists` check in the execute method before writing.\n\nVerification: Add a test that creates two perspectives with the same name and verifies the expected behavior." #review-finding