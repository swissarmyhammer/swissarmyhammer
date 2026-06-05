---
assignees:
- claude-code
depends_on:
- 01KTBNMJY54KG5K7BWG29C2J1J
- 01KTBN9E9FD9X1PY1ARY9SMN99
position_column: todo
position_ordinal: 8c80
project: local-review
title: 'Engine stage 4 — synthesize: dedup, severity-rank, render the dated GFM checklist'
---
## What
Final engine stage and the single barrier: the top-level `run_review` awaits the shared pool fully draining (all fan-out + verify tasks done), then calls `synthesize(verified, now)` to turn `Vec<VerifiedFinding>` into the deduped, severity-ranked report in the EXACT format the existing review skill already writes on kanban tasks. Pure/deterministic — no agents.

- **Drop refuted** findings (`confirmed == false`).
- **Dedup conservatively — no fuzzy matching:**
  - Collapse only **exact repeats** (same `file`, `line`, `validator`, `rule`) into one.
  - Do NOT merge across validators. If `duplication` and `dead-code` both flag the same line, those are distinct lenses — keep both.
  - **Group by `file:line`** in the render so co-located findings appear together (grouping ≠ merging — no concern is silently dropped).
  - (Optional, conservative) collapse two findings only when their `claim` text is byte-identical.
- **Group by severity** into `Blockers` / `Warnings` / `Nits`; one concern per checklist item; omit empty sections.
- **Render** the dated GFM section verbatim to the existing skill format:
  `## Review Findings (YYYY-MM-DD HH:MM)` → `### Blockers` / `### Warnings` / `### Nits`, each item `- [ ] \`file:line\` — claim. suggestion.` (timestamp passed in by the caller; the engine never reads the clock).
- Return `ReviewReport { markdown, counts{ blockers, warnings, nits, confirmed, refuted } }` for the tool/skill.

## Acceptance Criteria
- [ ] `synthesize(verified, now) -> ReviewReport` exists; refuted excluded; only exact `(file,line,validator,rule)` repeats collapsed; cross-validator findings preserved; severities grouped; empty sections omitted; findings ordered/grouped by `file:line` within a severity.
- [ ] The rendered markdown matches the current review skill's section format exactly (existing task-history parsing keeps working).
- [ ] Timestamp is an input parameter, not read inside the engine (deterministic/testable).
- [ ] No fuzzy/similarity-based dedup anywhere.

## Tests
- [ ] Unit test: a set of verified findings (incl. one refuted → dropped, one exact repeat → collapsed, two different validators on the same `file:line` → both kept and grouped) → correct markdown + counts.
- [ ] Snapshot test of the rendered section against the documented format.
- [ ] `cargo test -p swissarmyhammer-validators review::synthesize` green.

## Workflow
- Use `/tdd` — assert the rendered markdown + counts for a hand-built `Vec<VerifiedFinding>` first. Match the format in `builtin/skills/review/SKILL.md` step 8 byte-for-byte; do not invent a new layout. No similarity libraries — exact-match dedup only.