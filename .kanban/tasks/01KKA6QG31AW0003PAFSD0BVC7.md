---
position_column: done
position_ordinal: x8
title: DeriveRegistry missing Debug impl
---
swissarmyhammer-fields/src/derive.rs:59\n\n`DeriveRegistry` is a public type but does not implement `Debug`. Per Rust review guidelines, all public types should implement `Debug` (and other applicable traits like `Default`). `Default` is implemented, but `Debug` is missing.\n\nThe `Box<dyn DeriveHandler>` prevents `#[derive(Debug)]`, but a manual impl that prints handler names (the keys) would suffice.\n\nSuggestion: Add a manual `Debug` impl that shows registered handler names:\n```rust\nimpl std::fmt::Debug for DeriveRegistry {\n    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {\n        f.debug_struct(\"DeriveRegistry\")\n            .field(\"handlers\", &self.handlers.keys().collect::<Vec<_>>())\n            .finish()\n    }\n}\n```" #review-finding #warning