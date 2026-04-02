---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffff9580
title: 'NIT: store_name() default falls back to \"unknown\" for pathological roots'
---
swissarmyhammer-store/src/store.rs:51-56\n\nThe default store_name() implementation:\n```rust\nfn store_name(&self) -> &str {\n    self.root()\n        .file_name()\n        .and_then(|n| n.to_str())\n        .unwrap_or(\"unknown\")\n}\n```\n\nReturning \"unknown\" silently masks the problem. If a store root is \"/\" or a non-UTF-8 path, every event from that store will have store_name=\"unknown\", which will not match any entity_type in the bridge. The EntityTypeStore correctly overrides this (returning entity_type_name), so this only affects custom TrackedStore implementations that forget to override.\n\nSuggestion: This is minor since EntityTypeStore overrides it. Consider logging a warning in the default implementation when falling back to \"unknown\", or make store_name() a required method with no default (forcing implementors to think about it).",
<parameter name="tags">["review-finding"] #review-finding