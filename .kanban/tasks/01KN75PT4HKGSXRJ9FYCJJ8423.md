---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffd380
title: 'Test git repository: error paths and utility functions'
---
File: swissarmyhammer-git/src/repository.rs (48.8%, 43 uncovered lines)

Uncovered areas:
- GitRepository::open() error branches: Invalid, Ambiguous error codes
- contains_path() for bare repositories (else branch)
- state() / is_in_normal_state() during merge/rebase
- utils::get_git_dir()
- find_from_current_dir()

Also swissarmyhammer-git/src/types.rs (68.8%, 30 uncovered lines):
- StatusSummary::all_changed_files() 
- StatusSummary::has_conflicts()
- CommitInfo::new() with short hash < 8 chars
- FromStr impl for BranchName #coverage-gap