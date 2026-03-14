---
position_column: done
position_ordinal: z00
title: '[nit] Treesitter integration tests use `unwrap()` throughout — contradicts doc guidelines'
---
**File:** `swissarmyhammer-treesitter/tests/workspace_leader_reader.rs`\n**Severity:** nit\n\nThe Rust review guidelines state \"Examples use `?`, not `.unwrap()`.\" While tests are not documentation examples, consistent use of `unwrap()` without any message makes test failure output unhelpful (\"called `Option::unwrap()` on a `None` value\" with no context). The file uses `.unwrap()` on ~20 call sites.\n\nFor tests, prefer `.expect(\"descriptive message\")` at every `unwrap()` call so failures identify *what* was expected. For example:\n```rust\nlet workspace = Workspace::new(dir).open().await.expect(\"workspace should open\");\n```\n\nThe assertion messages in `assert!` and `assert_eq!` are already good — just the unwrap sites need messages." #review-finding