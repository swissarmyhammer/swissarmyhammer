---
position_column: done
position_ordinal: fffffff580
title: 'swissarmyhammer-treesitter: 7 workspace_leader_reader integration tests fail with background indexing timeout'
---
All 7 integration tests in swissarmyhammer-treesitter::workspace_leader_reader panic at workspace_leader_reader.rs:38 with 'background indexing did not complete within timeout: Elapsed(())'. Affected tests: test_leader_can_query_duplicates, test_leader_creates_database, test_leader_indexes_files, test_new_leader_can_reopen_existing_database, test_reader_can_open_database_readonly, test_reader_can_query_chunks, test_reader_cannot_run_tree_sitter_queries #test-failure