---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffe080
title: 'warning: tests in avp-common/src/install.rs missing coverage for Local scope and User scope settings path'
---
avp-common/src/install.rs:299-501\n\nThe test suite is solid but has two gaps:\n\n1. `test_settings_path_local` and `test_settings_path_project` test the relative path return values, but there is no test for `settings_path(InitScope::User)`. Since that path depends on `dirs::home_dir()`, a test that asserts it ends with `.claude/settings.json` (without checking the prefix) would catch regressions.\n\n2. All `install`/`uninstall` integration tests use `InitScope::Project`. There are no tests exercising `InitScope::Local` (which uses `.claude/settings.local.json`). The Local path through `install` is structurally identical, but the `.avp` directory creation branch (`matches!(scope, InitScope::Project | InitScope::Local)`) is untested for Local.\n\nSuggestion: Add `test_install_local_scope` mirroring `test_install_creates_hooks_and_avp_dir` with `InitScope::Local`, and a `test_settings_path_user` that checks the suffix.\n\nVerification: `cargo nextest run -E 'package(avp-common)'` with new tests passes." #review-finding