---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffea80
project: ai-panel
title: Qwen over-scopes simple "create a task" requests — loads explore skill + whole-repo grep instead of just adding the card
---
## P2 — behavioral: the agent walked itself into the grep wedge

### Evidence
Asked only to "make a task to eliminate unused partials," the qwen agent (11:38:25→53) first called the `skill` tool twice to load the **`explore`** skill, then ran three `code_context` symbol searches *and* a whole-repo `grep_files` content search — all before ever calling kanban `add task`. That expensive unscoped grep is what it hung on. A task-creation request should not trigger deep code investigation.

### Fix direction
- Tune the AI-panel system prompt / tool-selection guidance so simple "create/add a task" intents resolve to a direct `kanban add task` with light (or no) code lookup, and do **not** auto-load the `explore` skill.
- Consider a cheaper default: if exploration is wanted, prefer scoped, bounded searches over `output_mode:"content"` whole-tree greps.

### Priority
Secondary to the P0/P1 reliability cards — once the grep root + watchdog + session-abort fixes land, this stops being able to hang anything; this card just keeps the agent from wasting time/context on over-exploration.