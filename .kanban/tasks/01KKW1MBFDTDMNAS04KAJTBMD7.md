---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffbf80
title: 'warning: uninstall always removes .avp directory regardless of scope'
---
avp-common/src/install.rs:193-241\n\n`uninstall` unconditionally removes the `.avp` directory (lines 233-238) regardless of the scope passed. For `InitScope::User`, the `.avp` directory removal uses `base_dir.join(\".avp\")`. Since `base_dir` for the User scope is `current_dir()` (as passed by both callers), and the function does not guard this removal behind a scope check, calling `avp uninstall --user` from inside a project directory will silently delete that project's `.avp` directory.\n\nThis is the same scope-guard asymmetry as the install path, but worse: install conditionally creates `.avp` only for Project/Local (line 168), but uninstall removes it unconditionally. These two should be symmetric.\n\nSuggestion: Wrap the `.avp` removal in `if matches!(scope, InitScope::Project | InitScope::Local)` to mirror the install path.\n\nVerification: Add a test `test_uninstall_user_scope_does_not_remove_avp_dir` that creates a `.avp` directory, calls `uninstall(InitScope::User, base_dir)`, and asserts the directory still exists." #review-finding