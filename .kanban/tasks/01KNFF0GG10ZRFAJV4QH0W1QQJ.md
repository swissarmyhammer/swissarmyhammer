---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffe780
title: ReferencesResult and ReferenceLocation missing Deserialize derive
---
swissarmyhammer-code-context/src/ops/get_references.rs\n\nThe result types `ReferencesResult`, `ReferenceLocation`, and `FileReferenceGroup` derive `Serialize` but not `Deserialize`:\n\n```rust\n#[derive(Debug, Clone, Serialize)]\npub struct ReferenceLocation { ... }\n```\n\nAll other result types in the new ops (GetDefinitionResult, HoverResult, InboundCallsResult, etc.) derive both `Serialize` and `Deserialize`, and have round-trip serialization tests. This is inconsistent and prevents consumers from deserializing the result.\n\nSuggestion: Add `Deserialize` to the derive list for all three types and add a round-trip serialization test to match the pattern in other ops." #review-finding