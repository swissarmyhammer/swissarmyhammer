---
position_column: done
position_ordinal: q4
title: '[WARNING] WorkspaceMode missing Debug impl'
---
File: swissarmyhammer-code-context/src/workspace.rs\n\nWorkspaceMode is a public enum but has no Debug implementation. Connection and LeaderGuard may not be Debug, but a manual impl (e.g., printing 'Leader' or 'Reader') should be provided.\n\nCodeContextWorkspace is also missing Debug. #review-finding