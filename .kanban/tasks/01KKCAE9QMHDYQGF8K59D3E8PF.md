---
depends_on:
- 01KKCADZV71JJH01V5GVEPQTAX
position_column: done
position_ordinal: ffffffffffb980
title: 'STATUSLINE-M1: directory module'
---
## What\nModule: `directory`. Source: `input.workspace.current_dir`. Default format: `$path`.\n\nExtracts basename of cwd. Config: `truncation_length` (default 1 = last component), `style` (default \"cyan bold\").\n\nFile: `swissarmyhammer-statusline/src/modules/directory.rs`\n\n## Acceptance Criteria\n- [ ] Shows basename of workspace dir\n- [ ] Configurable format, style, truncation_length\n- [ ] Returns None if no cwd available\n\n## Tests\n- [ ] Test with full path, verify basename\n- [ ] Test with absent cwd returns None\n- [ ] Test custom format string