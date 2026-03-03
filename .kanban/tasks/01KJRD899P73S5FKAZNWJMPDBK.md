---
title: Duplicate Locking section header in context.rs
position:
  column: todo
  ordinal: c8
---
**File:** `swissarmyhammer-kanban/src/context.rs` around line 970\n\n**What:** There is a duplicate section separator comment block for \"Locking\":\n```\n    // =========================================================================\n\n    // =========================================================================\n    // =========================================================================\n    // Locking\n    // =========================================================================\n```\n\nThe first empty section header is a leftover from refactoring. There should be only one Locking section header.\n\n**Why it matters:** Minor readability issue. The extra empty section header is confusing when scanning the file structure.\n\n**Suggestion:**\n- [ ] Remove the empty `// =========================================================================` block (the one before the duplicate)\n- [ ] Keep only the final `// Locking` header\n- [ ] Verify formatting with `cargo fmt`\n\n#warning #warning