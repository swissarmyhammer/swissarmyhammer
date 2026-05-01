---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffda80
title: Make mirdan uninstall_skill pub
---
Change `fn uninstall_skill()` at mirdan/src/install.rs:877 from private to `pub fn` so ShellExecuteTool::deinit() can call it synchronously.\n\n## Files\n- mirdan/src/install.rs\n\n## Acceptance\n- `uninstall_skill` is pub\n- Existing callers (run_uninstall) still work\n- Tests pass