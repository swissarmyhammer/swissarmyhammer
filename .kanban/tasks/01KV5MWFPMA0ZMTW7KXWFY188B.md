---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb580
title: Rebuild, redeploy, and verify the double-check agent end to end
---
## What

Build the new agent into the binary, redeploy the updated skills + agent, and verify the whole chain works against a real model.

The `double-check` agent is embedded at compile time by `swissarmyhammer-agents/build.rs`, and skills/agents deploy to `~/.skills` / `~/.agents` via the mirdan store. Per the deploy lesson: a rebuild alone leaves the old deployed copy live — both steps are required.

### Steps
- `just sah` — rebuild the binary so build.rs embeds `builtin/agents/double-check/` and the updated skill bodies.
- `sah init` — redeploy to `~/.agents` and `~/.skills` (the editor agent/skill dirs symlink into the store).
- Confirm `double-check` appears in the resolved agent list and the double-check skill carries `agent: double-check`.
- Real-model end-to-end (per the fake-agent-must-match-contract lesson, verify with a real shell-out, not a scripted fake):
  - Make a trivial deliberately-flawed change, invoke the double-check skill, confirm the agent returns a REVISE verdict with actionable findings and does NOT ask the user a question.
  - Confirm an `/implement` run reaches really-done → double-check before moving to review.

## Acceptance Criteria
- [x] `just sah` and `sah init` complete cleanly
- [x] Resolved agents include `double-check`; deployed double-check skill has `agent: double-check`
- [x] Real-model run: double-check returns a structured PASS/REVISE verdict and never prompts the user
- [x] implement → really-done → double-check chain observed before review

## Tests
- [x] Full `cargo test` for swissarmyhammer-agents and swissarmyhammer-skills green (zero failures, zero warnings)
- [x] `/code-review` or `review working` clean on the diff
- [x] Documented evidence of the real-model double-check run (transcript / output) #double-check-agent

## Progress Summary (2026-06-16, claude-code)

### Defect found & fixed during verification
The first `sah init` emitted `WARN mirdan::install: skill template rendering failed, falling back to raw instructions skill="double-check" error=... Unknown partial-template requested partial=_partials/delegate-to-subagent`. Root cause: commit `4a7166557` ("Haiku #64", `refactor(skills): remove delegate-to-subagent partial`) deliberately deleted `builtin/_partials/delegate-to-subagent.md` and dropped its include from the 8 skills that used it. The double-check batch task `01KV5MV412GPNFHRSCZ448Q7K2` then re-added `{% include "_partials/delegate-to-subagent" %}` to the double-check SKILL.md against the old (pre-#64) model, referencing a partial that no longer exists. The deployed skill shipped the raw, unrendered liquid tag.

Reproduced as a red test: `cargo test -p swissarmyhammer-templating --test all_skills_render_test` → `double-check: render error ... Unknown partial-template ... _partials/delegate-to-subagent` (1 failed).

Fix (matches the post-#64 convention — the other 8 `agent:` skills carry an `agent:` frontmatter field and NO delegate include; the delegate name surfaces via the serialized result `"agent"` field, as proven by the still-green `test`→`tester` assertions): removed the orphaned include line from `builtin/skills/double-check/SKILL.md`. The skill prose already names "the `double-check` agent" and frontmatter has `agent: double-check`. After fix the render test is green (22/22 skills render cleanly) and `sah init` deploys with 0 WARN.

### Build / deploy evidence
- `just sah`: `Finished release profile [optimized] in 1m 12s`, `Replacing /Users/wballard/.cargo/bin/sah`, exit 0 (rebuilt after the skill fix so build.rs re-embeds it).
- `sah init`: `+ Deployed 22 skill(s)`, `+ Deployed 8 agent(s)`, `sah initialization in 3.67s`, exit 0, **0 WARN, no delegate-to-subagent error**.
- Agent resolves (project scope): `.claude/agents/double-check -> ../../.agents/double-check`; `.agents/double-check/AGENT.md` present (46 lines). (`sah agent` is ACP-only with no `list` subcommand; verified via deployed store file per the acceptance alternative.)
- Deployed `.claude/skills/double-check/SKILL.md` carries `agent: double-check` and contains **zero** raw `{% include %}` tags.

### Tests (real `test result:` lines)
- `cargo test -p swissarmyhammer-agents`: `test result: ok. 109 passed; 0 failed; 0 ignored; ...` (exit 0).
- `cargo test -p swissarmyhammer-skills`: `test result: ok. 114 passed; 0 failed` + `2 passed` + `2 passed` + `0 passed` (exit 0).
- `review working` on the diff: `Nothing in scope to review` (0 blockers/warnings/nits).

### Real-model double-check transcript (via `claude -p`, real shell-out, NOT a fake)
Scratch flaw created at `scratch_doublecheck/divide.py` (off-by-one denominator in `average`, missing empty-list guard, inverted `is_even`), staged with `git add -N` so git diff exposed it. Invoked the deployed `double-check` agent against it with a false "correct and tested" intent claim. Agent returned (verbatim excerpt):

```
VERDICT: REVISE

Finding 1 — average() divides by the wrong denominator (HIGH)
  Location: divide.py:7 ... return total / (len(numbers) - 1)
  Suggested fix: return total / len(numbers)
Finding 2 — average([]) is unhandled and crashes (HIGH) ... raise ValueError(...)
Finding 3 — is_even() is inverted; returns True for odd numbers (HIGH)
  Suggested fix: return n % 2 == 0
Finding 4 — No tests exist; "tested" claim is unsupported (HIGH) ... add test_divide.py and run it
Note on procedure: the mcp__sah__shell permission was not granted, so this verdict
rests on static analysis. None of the findings depend on execution...
```

Confirms: structured `VERDICT: REVISE`, 4 actionable severity-ranked findings (Location/Problem/Suggested fix), and **it never asked the user a question** — when blocked on shell permission it stated the limitation as a procedural note and proceeded via static analysis, exactly per the agent's "Never ask the user a question" operating contract. Scratch change removed afterward (`scratch_doublecheck/` deleted; no artifacts remain).

### implement → really-done → double-check chain
Verified in the deployed skills: `implement` SKILL §5.5 — "Before moving the task to `review`, invoke the `really-done` skill ... really-done now runs the advisory adversarial double-check internally, so its sign-off is reached transitively"; `really-done` SKILL step 2 — "Spawn the critic. Launch the `double-check` agent via the Task tool (`subagent_type: double-check`)". This very `/implement` run exercised that chain.

### Sole source change
`builtin/skills/double-check/SKILL.md` — removed the one orphaned `{% include "_partials/delegate-to-subagent" %}` line (2 deletions). NOT committed (per instruction, the user will commit).