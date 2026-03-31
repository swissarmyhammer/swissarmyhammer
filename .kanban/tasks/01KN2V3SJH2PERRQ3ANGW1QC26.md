---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffc280
title: '[WARNING] claude_settings_path panics on missing home directory'
---
At line 418, `claude_settings_path` calls `dirs::home_dir().expect(\"home directory required\")` for the `User` scope variant. This is a panic on an expected failure mode (missing HOME env var, unusual system configurations, CI environments). Per Rust review guidelines: 'Panics are for bugs only -- internal invariant violations. Never panic on expected failure modes.'\n\nThe function should return `Result<PathBuf>` instead, or at minimum the callers (`init`/`deinit`) already return `Vec<InitResult>` which can carry errors gracefully.\n\nNote: `is_applicable` currently filters out `User` scope, so this panic is currently unreachable in practice. But the function is not guarded by visibility -- any caller could pass `InitScope::User` and trigger it. This is a latent defect.\n\nFile: `swissarmyhammer-tools/src/mcp/tools/shell/mod.rs`, lines 417-420 #review-finding