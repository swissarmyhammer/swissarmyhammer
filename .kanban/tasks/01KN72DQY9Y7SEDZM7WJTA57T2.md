---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9280
title: 'WARNING: No test verifying store_name appears in flush_changes event payloads'
---
swissarmyhammer-store/src/handle.rs:957-993\n\nThe flush_changes tests (flush_changes_detects_external_create, flush_changes_detects_external_change, flush_changes_detects_external_remove) verify event_name and payload[\"id\"] but never assert on payload[\"store\"]. The store_name enrichment is the critical new feature of this branch, yet no test verifies the store name flows through correctly.\n\nThe MockStore in tests uses the default store_name() implementation which infers from the directory basename. If the store root is \"/tmp/.tmpXXXXX/store1\", store_name() returns \"store1\". But no test asserts this value.\n\nSuggestion: Add assertions like `assert_eq!(events[0].payload[\"store\"], \"store1\")` to each flush_changes test. Also add a test with a custom store_name() override to verify the EntityTypeStore path.",
<parameter name="tags">["review-finding"] #review-finding