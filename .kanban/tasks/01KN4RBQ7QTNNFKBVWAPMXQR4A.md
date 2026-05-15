---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffbe80
title: Hardening tests repeat boilerplate setup -- extract shared test fixture
---
**File**: `swissarmyhammer-shell/src/hardening.rs` lines 751-855\n**Layer**: Design / Maintainability\n**Severity**: Low\n\nThe four `test_validate_command_comprehensive_*` tests each repeat the same 5-line setup:\n```rust\nlet policy = ShellSecurityPolicy::default();\nlet config = SecurityHardeningConfig::default();\nlet mut validator = HardenedSecurityValidator::new(policy, config);\nlet tmp_dir = tempfile::TempDir::new().unwrap();\nlet env = HashMap::new();\n```\n\nThis is a textbook case for a test helper function (similar to `make_metric` and `profiler_with_injected_metrics` already used in performance.rs).\n\n**Recommendation**: Extract a helper like `fn default_validator_with_tmpdir() -> (HardenedSecurityValidator, TempDir)` to reduce repetition and make future test additions cheaper. #review-finding