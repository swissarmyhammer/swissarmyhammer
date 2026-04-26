---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffda80
title: 'Test git2_utils: add_files and create_commit'
---
File: swissarmyhammer-git/src/git2_utils.rs (0%, 40 uncovered lines)

Both public functions completely untested:
- add_files(repo, paths) - adds files to index
- create_commit(repo, message, author_name, author_email) - creates a commit with optional author override

Need tempdir integration tests with a real git repo. Test both the default-signature and explicit-signature paths for create_commit. #coverage-gap