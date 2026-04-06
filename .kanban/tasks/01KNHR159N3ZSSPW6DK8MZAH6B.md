---
assignees:
- claude-code
position_column: todo
position_ordinal: bc80
title: 'Coverage: config.rs — load_code_context_config and merge_config_stack edge cases'
---
swissarmyhammer-code-context/src/config.rs

Coverage: 77.8% (42/54)

Most config parsing and merging is tested. The gaps are in the real config loader and edge cases in the merge stack.

Uncovered functions/branches:

1. `load_code_context_config()` (lines 98-111) — The production config loader that uses VirtualFileSystem with dot-directory paths. Never called in tests (only `load_code_context_config_from_paths` is tested). The `vfs.load_all()` error branch (lines 103-108) is uncovered.

2. `merge_config_stack` None branch (lines 148-151) — When `vfs.get_stack(CONFIG_NAME)` returns `None` (no config found at all, not even builtin). This triggers a fallback to parsing BUILTIN_CONFIG_YAML directly. Currently unreachable in tests because the builtin is always added.

3. `load_code_context_config_from_paths` FileSource assignment (lines 124-128) — The branch logic for assigning `FileSource::User` vs `FileSource::Local` based on position. The `User` case (first path when multiple paths provided) is never tested — all tests pass either 0 or 1 overlay paths.

4. `PatternCompileError` Display — The `reason` field on PatternCompileError is set but never asserted in tests.

What to test:
- Test `load_code_context_config_from_paths` with 2+ overlay dirs to exercise the FileSource::User branch for the first path.
- Test `merge_config_stack` with a VFS that has no entries for CONFIG_NAME (construct a VFS manually, don't add builtin, call merge_config_stack).
- Test `load_code_context_config()` in a controlled environment (set HOME to a temp dir, verify it loads without errors). This may require mocking or setting env vars.
- Assert `PatternCompileError.reason` field in the existing test_invalid_regex_compile_error test.

#coverage-gap #code-context