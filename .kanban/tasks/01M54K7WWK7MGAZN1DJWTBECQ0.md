---
assignees:
- claude-code
depends_on:
- 01M53GC28P974P34HEP7XN651M
position_column: todo
position_ordinal: ffff8380
title: Make the watcher row key from the same root form as the walk
---
## Problem
`to_file_event` (`crates/swissarmyhammer-tools/src/mcp/tools/code_context/watcher.rs`) makes the row key with `path.strip_prefix(workspace_root).unwrap_or(&path)`. FSEvents on macOS can send the canonical path (`/private/var/x/...`) when the root is `/var/x`, or when the root is a symlink. `IgnoreFilter::relative` accepts such a path through the canonical root, but `to_file_event` does not strip it. The row key then becomes the absolute path. The startup walk and the reconcile walk (`cleanup.rs` `walk_and_hash`) make the key relative to the root. Thus one file can get two rows: `src/a.rs` and `/private/var/x/src/a.rs`. The reconcile deletes the absolute row (the walk does not return it), and the watcher adds it again on the next event.

The Swift port session found the same problem in its watcher. In that watcher, all events are dropped and no log is written.

## Fix
- Make one function that gives the relative path of an event path. It must try the root, then the canonical root, the same as `IgnoreFilter::relative`. Make that function public (or move it to a shared place) and use it in `to_file_event` and in the diagnostics watcher.
- If a path is outside both roots, drop the event and write a debug log. Do not use the absolute path as a row key.
- Check that the startup walk, the reconcile and the diagnostics scope all make keys from the same root form.

## Acceptance criteria
- With a symlinked root, an event for `<canonical root>/src/a.rs` gives the row key `src/a.rs`.
- An event for a path outside both roots adds no row.

## Tests
- A unit test for `to_file_event` (or `collect_file_events`) with a symlinked temp root. Use the same `symlinked_root` helper as in `ignore_filter.rs`. #lsp-fix #bug