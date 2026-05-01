---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffff280
title: Add tests for DefaultProvider and CliProvider load_into/metadata
---
swissarmyhammer-config/src/provider.rs:262-304\n\nUncovered lines: 270-271, 287, 289, 302-303\n\n```rust\nimpl ConfigurationProvider for DefaultProvider { fn load_into, fn metadata }\nimpl ConfigurationProvider for CliProvider { fn load_into, fn metadata }\n```\n\nDefaultProvider::metadata returns \"defaults\", CliProvider::metadata returns \"cli\". Their load_into methods merge via Serialized::defaults. The existing tests cover the providers partially but miss the metadata() return values and the CliProvider::empty() path. Test: verify metadata names, test CliProvider::empty produces an empty figment, test DefaultProvider::empty produces an empty figment. #Coverage_Gap