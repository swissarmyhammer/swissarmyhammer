---
assignees:
- claude-code
depends_on:
- 01KTBNNTCCVS81QZV4CFQZV4X1
- 01KTC0MZ8ZH46WCN514J4RPS3H
position_column: done
position_ordinal: fffffffffffffffffffffffffffffffffffffe80
project: local-review
title: Rewrite the `review` skill + `reviewer` agent as thin drivers of the review tool
---
## What
Move the heavy reasoning out of the skill into the engine. `builtin/skills/review/SKILL.md` stops being a 7-layer manual checklist and becomes a thin driver: detect mode, call the right `review` op, record the returned findings on kanban, summarize.

- **Keep verbatim** (these already work): mode detection (task-mode vs range-mode), the review-column gate, the task-mode/range-mode kanban write logic (append dated `## Review Findings` section / create tracking task / move to terminal when clean), and "column movement is the verdict."
- **Map modes onto the tool's verb-noun ops:**
  - task-mode (`/review <id>` or oldest task in the `review` column) → read the task, derive scope from any range hint in its body → `review sha <range>`; no hint → `review working`.
  - range-mode → `review working` (default), `review sha <range>` (commit/range), or `review file <path|glob>`.
  - The skill passes the chosen op to the `review` tool, takes `ReviewReport.markdown`, and writes it into the task per the existing contract; uses `counts` for the summary.
- **Drop the manual analysis** (old Process steps 3–7: get-changes / read-every-file / 7 layers / language refs). The engine fleet does all of it.
- **Remove the language reference files entirely** — `references/RUST_REVIEW.md` etc. are now language validators (separate task). Delete the `references/*_REVIEW.md` links and the "apply language guidelines" step from the skill.
- **Document the passthrough**: `validators` (subset — "review just duplication") and `backend` ("review locally" → force local Llama). No "dimensions" (gone).
- Update `builtin/agents/reviewer/AGENT.md` to the thin-driver flow; keep `code-context`, `really-done`, `thoughtful` deps; trim manual-layer instructions.
- Regenerate `.skills/` from `builtin/skills/` (generated dir — never hand-edit).

## Acceptance Criteria
- [ ] `builtin/skills/review/SKILL.md` drives the `review` tool for analysis (maps task/range modes to `review file/working/sha`) and keeps the mode-detection + kanban-write contract verbatim; it no longer hand-runs layers or links `*_REVIEW.md`.
- [ ] `validators` + `backend` passthrough documented; no "dimensions".
- [ ] `reviewer` AGENT.md reflects the thin-driver flow.
- [ ] `.skills/` regenerated from source; the `references/*_REVIEW.md` links are gone.

## Tests
- [ ] Skill-render/lint test (the repo's skill frontmatter/template validation) passes for the rewritten skill.
- [ ] Doc/lint check: the skill references real tool ops (`review working`/`review sha`/`review file`) and no longer references `references/*_REVIEW.md`; generated `.skills/review` matches source.

## Workflow
- Skill-authoring task. Preserve the kanban write contract byte-for-byte; only swap the analysis engine and drop the language refs. Do NOT hand-edit `.skills/` — edit `builtin/skills/review/` and regenerate. Depends on the review tool (ops exist) and the language-validators task (so the language content is migrated before the refs are deleted).