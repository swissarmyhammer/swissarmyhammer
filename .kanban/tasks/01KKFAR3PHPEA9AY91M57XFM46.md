---
position_column: done
position_ordinal: ffb480
title: '[nit] Missing Debug on LspServerSpec (static version)'
---
**Severity: nit**\n**File:** swissarmyhammer-lsp/src/types.rs:10\n\nPer Rust review guidelines, all public types should derive Debug. `LspServerSpec` has a manual `Debug` impl (good, since it contains a function pointer), but the struct itself lacks a `#[derive(Debug)]` annotation -- it relies on the manual impl. This is actually correct since you cannot derive Debug on fn pointers. No action needed, but noted for completeness." #review-finding