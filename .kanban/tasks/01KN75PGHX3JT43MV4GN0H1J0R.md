---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9380
title: 'Test git operations: branch management and merge'
---
File: swissarmyhammer-git/src/operations.rs (12.9%, 392 uncovered lines)

Uncovered functions needing tests:
- get_changed_files_from_parent() - diff against parent branch via merge-base
- get_changed_files_from_range() - diff within a revision range
- get_all_tracked_files() - walk HEAD tree for all blobs
- branch_exists() / checkout_branch() / delete_branch()
- merge_branch() - full merge logic
- find_merge_target_for_issue() - heuristic merge target resolution
- validate_branch_creation()
- has_uncommitted_changes() / checkout_branch_str() / delete_branch_str() / branch_exists_str()
- commit() / add_all()

These are real git operations that need tempdir-based integration tests with actual repo fixtures. Focus on the multi-commit branch scenarios first. #coverage-gap