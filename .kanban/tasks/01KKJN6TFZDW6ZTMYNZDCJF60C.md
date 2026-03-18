---
position_column: done
position_ordinal: ffffffde80
title: '[nit] `ElectionConfig::base_dir()` (private getter) clones `PathBuf` unnecessarily'
---
**File:** `swissarmyhammer-leader-election/src/election.rs` line 60\n**Severity:** nit\n\n```rust\nfn base_dir(&self) -> PathBuf {\n    self.base_dir.clone().unwrap_or_else(std::env::temp_dir)\n}\n```\n\nThis always clones the `Option<PathBuf>` even in the `Some` branch. Prefer:\n\n```rust\nfn base_dir(&self) -> PathBuf {\n    self.base_dir.as_deref()\n        .map(PathBuf::from)\n        .unwrap_or_else(std::env::temp_dir)\n}\n```\n\nOr simply keep the clone but acknowledge it's a minor allocation on the hot path of initialisation (not in a loop). Low impact, but the pattern teaches the wrong idiom." #review-finding