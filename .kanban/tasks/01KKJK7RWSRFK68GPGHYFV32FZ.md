---
position_column: done
position_ordinal: ffffff80
title: CodeContextWorkspace struct doc comment refers to \"reader\" not \"follower\"
---
swissarmyhammer-code-context/src/workspace.rs:59\n\nThe `CodeContextWorkspace` struct has a field comment `/// The mode (leader or reader)` which was not updated when `WorkspaceMode::Reader` was renamed to `WorkspaceMode::Follower`. This is a stale comment that will confuse readers.\n\nSuggestion: Change to `/// The mode (leader or follower)`.",
<parameter name="tags">["review-finding"] #review-finding