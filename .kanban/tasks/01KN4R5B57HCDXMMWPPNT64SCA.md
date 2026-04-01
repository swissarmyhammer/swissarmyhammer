---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffcc80
title: test_resolve_global_dir_uses_home mutates HOME env var without serial_test guard
---
swissarmyhammer-config/src/discovery.rs:310-327\n\nThe test `test_resolve_global_dir_uses_home` calls `std::env::set_var(\"HOME\", ...)` without any serialization guard. Other tests in the same crate (in file_discoverys.rs) correctly use `#[serial_test::serial(cwd)]` and a mutex for exactly this reason.\n\nThis is a race condition: if another test reads HOME concurrently, it will see the temporary path, causing intermittent failures. The test also does not restore HOME if an assertion panics before the restore code runs.\n\nSuggestion: Add `#[serial_test::serial(cwd)]` to the test attribute, or better yet, restructure the test to use the `IsolatedDiscoveryTest` helper from the integration tests which handles HOME save/restore in a Drop impl. #review-finding