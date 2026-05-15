---
assignees:
- claude-code
position_column: done
position_ordinal: fffffffffffffffffffffffffff780
title: ChangeEvent has public fields -- violates future-proofing guideline
---
swissarmyhammer-store/src/event.rs\n\n`ChangeEvent` is a public type with two public fields (`event_name: String` and `payload: serde_json::Value`). The Rust review guidelines require private struct fields on public types to avoid semver hazards. Adding a new field (e.g. `timestamp`) would be a breaking change.\n\nSuggestion: Make fields private, add getters (`event_name()`, `payload()`), and a constructor or builder. Alternatively, annotate with `#[non_exhaustive]` to reserve the right to add fields. #review-finding