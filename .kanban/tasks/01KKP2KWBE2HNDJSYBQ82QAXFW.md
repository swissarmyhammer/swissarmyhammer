---
position_column: done
position_ordinal: ffffffffb680
title: 'NIT: implement-loop description says "implement all planned kanban cards" but the skill operates on all non-done cards, not just "planned" ones'
---
builtin/skills/implement-loop/SKILL.md:3\n\nThe `description` field in the frontmatter reads: \"Implement all planned kanban cards autonomously until the board is clear.\" The word \"planned\" implies the skill only acts on cards in a \"Planned\" column, but the actual behaviour (via `/implement` → `kanban next task`) picks up any card that is ready regardless of column. This could mislead users who have cards in \"todo\" or \"doing\".\n\nSuggestion: Change description to \"Implement all ready kanban cards autonomously until the board is clear.\" (matching the body copy which says \"every kanban card\")." #review-finding