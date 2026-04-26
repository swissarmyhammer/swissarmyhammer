---
assignees:
- claude-code
position_column: done
position_ordinal: ffffff9980
title: 'heb/context.rs: test_context() function duplicates open() logic — test helper not testing the real code path'
---
heb/src/context.rs:109-125

The private `test_context()` helper in the unit tests re-implements `HebContext::open()` with different paths instead of providing a way to configure the paths on the real struct. This means the unit tests do not exercise the `open()` code path at all — bugs in `resolve_data_dir()` or `resolve_runtime_dir()` are invisible to tests.

Suggestion: add an `open_with_dirs(workspace_root, data_dir, runtime_dir)` constructor that the tests (and `open()`) both use. `open()` becomes a thin wrapper that calls `open_with_dirs` with the XDG-resolved paths. #review-finding