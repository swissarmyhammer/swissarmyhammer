---
assignees:
- claude-code
depends_on:
- 01KMTTV2PMJQ3GM2ED1B441N8Y
position_column: done
position_ordinal: ffffffffffea80
title: Register merge drivers in kanban InitBoard
---
## What\nWhen `init board` creates `.kanban/`, also configure git merge drivers scoped to that board's `.kanban/` directory.\n\n### Files to modify\n- `swissarmyhammer-kanban/src/board/init.rs` — in `InitBoard::execute()`, after creating directories and writing board entity, call a helper to register merge drivers\n- `swissarmyhammer-kanban/Cargo.toml` — may not need swissarmyhammer-merge dep since this is just git config manipulation\n\n### What init board should do (after existing setup)\n1. Check if `.git/` exists (walk up from board root). Skip silently if not in a git repo.\n2. Register three merge drivers in `.git/config`:\n   - `[merge \"kanban-jsonl\"]` driver = `kanban merge jsonl %O %A %B`\n   - `[merge \"kanban-yaml\"]` driver = `kanban merge yaml %O %A %B`\n   - `[merge \"kanban-md\"]` driver = `kanban merge md %O %A %B`\n3. Create/update `.gitattributes` with three patterns:\n   - `.kanban/**/*.jsonl merge=kanban-jsonl`\n   - `.kanban/**/*.yaml merge=kanban-yaml`\n   - `.kanban/**/*.md merge=kanban-md`\n4. Must be idempotent — running init twice doesn't duplicate entries\n\n### Key decisions\n- Driver names use `kanban-` prefix (not `sah-`) since the binary is `kanban`\n- The `.gitattributes` patterns are scoped to `.kanban/` — only kanban files use these drivers\n- Use data-driven loop over `(name, pattern, command)` tuples\n- No deinit needed yet (kanban has no deinit command)\n\n## Tests\n- `cargo nextest run -p swissarmyhammer-kanban init`\n- Test: init in temp git repo creates correct `.git/config` sections\n- Test: init in temp git repo creates correct `.gitattributes` lines\n- Test: double init is idempotent"}
</invoke>