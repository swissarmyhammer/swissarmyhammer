---
title: Update lib.rs docs and run full verification suite
position:
  column: done
  ordinal: c4
---
**Final card of the YAML/MD storage conversion plan.**

Update documentation and run the full verification checklist.

**lib.rs documentation:** Update the storage structure comment to reflect new formats:
```
repo/
└── .kanban/
    ├── board.yaml          # Board metadata (YAML)
    ├── board.jsonl          # Board operation log
    ├── tasks/
    │   ├── {id}.md          # Task (YAML frontmatter + markdown body)
    │   ├── {id}.jsonl       # Per-task operation log
    ├── tags/
    │   ├── {id}.yaml        # Tag state
    │   ├── {id}.jsonl       # Per-tag operation log
    ├── columns/
    │   ├── {id}.yaml        # Column state
    │   ├── {id}.jsonl       # Per-column operation log
    ├── swimlanes/
    │   ├── {id}.yaml        # Swimlane state
    │   ├── {id}.jsonl       # Per-swimlane operation log
    ├── actors/
    │   ├── {id}.yaml        # Actor state
    │   ├── {id}.jsonl       # Per-actor operation log
    └── activity/
        └── current.jsonl    # Global operation log
```

**Verification checklist:**
1. `cargo nextest run -p swissarmyhammer-kanban` — all tests pass
2. `cargo check -p swissarmyhammer-kanban-app` — Tauri app compiles
3. `npm run build` in `ui/` — TypeScript clean
4. Manually inspect `.kanban/tasks/*.md` — frontmatter + markdown body
5. Manually inspect `.kanban/tags/*.yaml` — YAML format
6. Inspect `.kanban/tags/*.jsonl` — logs present after ops
7. Inspect `.kanban/board.jsonl` — log entries after board ops
8. Test backward compat: existing `.json` files still readable

**What does NOT change:** JSONL log format, CLI/MCP JSON output, serde_json in operation execute methods.

**Files:**
- `swissarmyhammer-kanban/src/lib.rs` (doc update)

- [x] Update storage structure docs in lib.rs
- [x] Run cargo nextest run -p swissarmyhammer-kanban
- [x] Run cargo check -p swissarmyhammer-kanban-app
- [x] Run npm run build in ui/
- [x] Spot-check .md, .yaml, .jsonl file formats