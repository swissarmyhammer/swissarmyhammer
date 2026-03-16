---
position_column: done
position_ordinal: q3
title: '[WARNING] WorkspaceMode and CodeContextWorkspace have public fields'
---
File: swissarmyhammer-code-context/src/workspace.rs\n\nCodeContextWorkspace has two public fields: `pub mode: WorkspaceMode` and `pub workspace_root: PathBuf`. WorkspaceMode::Leader exposes `db: Connection` and `_guard: LeaderGuard` publicly.\n\nPer Rust review guidelines: 'Private struct fields. Public fields are a permanent commitment. Use getters/setters.'\n\nThe mode field is especially risky since callers could replace the WorkspaceMode and break the leader/reader invariant. The db() getter already exists, so mode does not need to be public. workspace_root could have a getter returning &Path. #review-finding