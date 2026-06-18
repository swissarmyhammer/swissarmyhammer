---
title: Double-Check the Card
description: Adversarial self-review of a freshly created kanban card before reporting done — re-read it, verify every named path/symbol exists, confirm sections, automated tests, sizing, and observable criteria, then fix-and-re-verify
partial: true
---

### Double-check the card before reporting done

A drafted card is not a finished card. After the task is persisted, run an
adversarial self-review against your own output — drafting and reviewing in one
pass misses stale paths, missing sections, vague criteria, manual-verification
tests, and oversized scope. Do NOT report back until the card passes every
check below.

**Do NOT spawn the diff-oriented `double-check` agent to verify a task card.**
That agent reviews a git diff and skips when there is no diff — a freshly
created card has no diff, so it is the wrong tool. The verification here is a
checklist you run yourself, not the diff critic.

Verify, in order:

1. **Re-read the created card.** Fetch the persisted card with
   `{"op": "get task", "id": "<id>"}` and review the actual stored text — not
   your memory of what you intended to write.

2. **Every named path, function, and type exists.** For each file path,
   function, or type the card references, confirm it is real — via the
   `code_context` MCP tool (`{"op": "search symbol", ...}` /
   `{"op": "get symbol", ...}`) or Glob/Read. This catches stale references and
   invented (hallucinated) names. A card that points at a path that does not
   exist sends the next agent down a dead end.

3. **All four required sections are present:** `## What`,
   `## Acceptance Criteria`, `## Tests`, and `## Workflow`. A card missing any
   of these is not actionable.

4. **The `Tests` section is automated.** No "manually verify…", "smoke test
   by…", "user confirms…", or any criterion whose only check is human
   observation. The standards above forbid this — confirm the produced card
   honors it.

5. **Sizing limits hold:** ≤5 files touched, ≤5 subtasks, one concern. More than
   that means multiple concerns — split along natural seams and link with
   `depends_on`.

6. **Acceptance criteria are observable, not vague.** Each criterion must name a
   concrete, checkable outcome ("returns `404` for unknown ids", "the new test
   passes") — not "works correctly" or "is improved".

**Fix-and-re-verify loop.** On any failure, correct the card with
`{"op": "update task", "id": "<id>", ...}` and then re-run the checks from the
top. Report the card as done only after it passes every check.
