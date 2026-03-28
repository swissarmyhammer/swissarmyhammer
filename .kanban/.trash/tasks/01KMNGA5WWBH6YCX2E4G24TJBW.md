---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
title: Make mirdan::install::uninstall_skill pub
---
Change `fn uninstall_skill()` at mirdan/src/install.rs:877 from private to `pub fn` so ShellExecuteTool::deinit() can call it synchronously.