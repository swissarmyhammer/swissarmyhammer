---
position_column: done
position_ordinal: ffac80
title: '[nit] ShellState::new_in_dir ignores its argument'
---
**Severity: nit**\n**File:** swissarmyhammer-tools/src/mcp/tools/shell/state.rs:110-113\n\nThe method `new_in_dir(_shell_dir: PathBuf)` takes a `_shell_dir` parameter but ignores it entirely, instead creating a new temp directory. The leading underscore signals intentional disuse, but this is confusing API: callers expect the supplied directory to be used. Either use the parameter or remove it and make the method private."