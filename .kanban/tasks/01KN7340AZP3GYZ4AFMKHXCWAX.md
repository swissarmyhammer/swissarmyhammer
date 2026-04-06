---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffe980
title: Add tests for ManagedDirectory XDG and git-root methods
---
src/directory.rs:103, 120-124, 139-142, 160-163, 188-189, 205-206, 222-223\n\nCoverage: 71.3% (62/87 lines)\n\nUncovered lines: 103 (write_gitignore error), 120-124 (write_gitignore logging), 139-142 (from_git_root), 160-163 (from_user_home), 188-189 (xdg_config write_gitignore), 205-206 (xdg_data), 222-223 (xdg_cache)\n\nThe directory-creation error path (line 93 → 103) and the gitignore write-error path need tests with read-only directories. from_user_home (deprecated) needs a basic coverage test. The XDG methods exist in tests but some inner branches aren't hit. #coverage-gap