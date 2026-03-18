---
assignees:
- claude-code
position_column: done
position_ordinal: fffff580
title: 'heb/context.rs: HebContext::open() ignores XDG dir for election, risking cross-workspace data sharing'
---
heb/src/context.rs:36-47

`HebContext::open()` creates the SQLite database in `XDG_DATA_HOME/heb/events.db` but this path is shared across ALL workspaces. Two different projects will publish into the same SQLite store, making replay-by-cwd the only filter. If the workspace root changes (rename, symlink), replays silently miss events.

The `data_dir` should be either workspace-scoped (e.g., `data_dir.join(hash_of_workspace)`) or the replay API should mandate a cwd filter so callers cannot accidentally read another workspace's events.

Suggestion: derive the db path from the workspace root hash, mirroring how leader-election derives socket paths. `XDG_DATA_HOME/heb/<workspace_hash>/events.db`. #review-finding