---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffc980
title: Add tests for PerspectiveContext::load_all edge cases
---
File: swissarmyhammer-perspectives/src/context.rs:178-202\n\nCoverage: 72.6% (53/73 lines in context.rs)\n\nUncovered lines: 180, 186, 196, 197\n\nThe load_all method has three uncovered branches:\n1. Line 180: root directory does not exist -- early return Ok(())\n2. Line 186: non-.yaml files in the directory are skipped\n3. Lines 196-197: invalid YAML files are logged and skipped\n\nWhat to test:\n1. Open a context where the root dir does not exist on the filesystem (distinct from create_dir_all creating it -- test the load_all early-return when root is gone between create and load).\n2. Place a non-.yaml file (e.g., .json, .txt) in the perspectives directory, open the context, and verify it is ignored.\n3. Place a malformed YAML file in the directory, open the context, and verify it is skipped without error and valid files still load. #coverage-gap