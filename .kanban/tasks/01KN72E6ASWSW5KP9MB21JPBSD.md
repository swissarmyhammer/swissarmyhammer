---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9380
title: 'WARNING: PerspectiveContext is not integrated with StoreHandle/StoreContext'
---
swissarmyhammer-perspectives/src/context.rs\n\nPerspectiveContext manages its own file I/O (atomic_write, fs::remove_file, load_all) independently of the StoreHandle system. This means:\n\n1. Perspectives have no undo/redo support -- writes go directly to disk with no changelog\n2. Perspectives are not detected by flush_changes() -- the StoreContext does not know about perspective files\n3. The file watcher will not detect perspective changes because \"perspectives\" is not in WATCHED_SUBDIRS (kanban-app/src/watcher.rs:21-28)\n\nThis creates an inconsistency: entity types go through StoreHandle (with undo, changelog, change detection), but perspectives bypass all of that. If a user adds a perspective and hits undo, nothing happens.\n\nSuggestion: Either implement TrackedStore for perspectives (making them first-class store citizens with undo/changelog) or document this as an intentional design decision. If intentional, add \"perspectives\" to WATCHED_SUBDIRS so the file watcher can at least detect external changes.",
<parameter name="tags">["review-finding"] #review-finding