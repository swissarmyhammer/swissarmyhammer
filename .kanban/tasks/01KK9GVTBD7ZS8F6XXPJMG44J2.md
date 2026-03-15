---
position_column: done
position_ordinal: ffffff9680
title: 'swissarmyhammer-treesitter: 14 unified::tests fail with background indexing timeout'
---
All 14 unit tests in swissarmyhammer-treesitter unified::tests panic at workspace_leader_reader.rs:38 with 'background indexing did not complete within timeout: Elapsed(())'. Affected tests: test_background_indexer_releases_lock, test_builder_with_progress_callback, test_check_file_unchanged_returns_path_for_unchanged, test_find_duplicates_in_file_with_file, test_incremental_indexing_mixed_changed_unchanged, test_incremental_indexing_new_file_added, test_incremental_indexing_reparses_changed_files, test_incremental_indexing_skips_unchanged_files, test_open_spawns_background_indexer, test_workspace_invalidate_file, test_workspace_list_files, test_workspace_open_becomes_leader, test_workspace_status, test_workspace_tree_sitter_query_fails_in_reader_mode #test-failure