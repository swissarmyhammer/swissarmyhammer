---
assignees:
- claude-code
depends_on:
- 01KTC7GT2DQ84KH443BRC75SHJ
position_column: todo
position_ordinal: '8780'
project: remove-prompts
title: Drop prompts from README, mdbook, and man pages
---
## What
Remove the "prompts" concept from user-facing documentation. Reframe sah's pitch around skills + workflows + agents. Distinguish sah-prompts (remove) from "system prompt" / "agent prompt turn" wording (keep where it refers to LLM prompting in agents).

Files to edit (markdown sources; the generated `doc/book/*.html` is rebuilt by mdbook, do not hand-edit HTML):
- `README.md` — remove prompt feature sections, `sah prompt` usage, prompt examples; keep skills/workflows.
- `doc/src/introduction.md` — drop the prompts framing.
- `doc/src/concepts/skills.md`, `doc/src/concepts/agents.md`, `doc/src/concepts/integrated-sdlc.md` — remove prompt references / comparisons that assume prompts exist as a feature (a brief "skills replaced prompts" note is fine if useful).
- `doc/src/reference/sah-cli.md` — remove the `prompt` command reference.
- `doc/src/reference/llama-claude-hooks.md`, `doc/src/reference/mirdan-cli.md` — scrub sah-prompt mentions; keep system/agent-prompt wording.
- `docs/sah.1`, `docs/mirdan.1` — remove the `prompt` subcommand from the man pages (or regenerate if generated).
- `doc/book.toml` / `doc/src/SUMMARY.md` — remove any prompts chapter entry.

Then rebuild the book: `cd doc && mdbook build` so `doc/book/` no longer references prompts.

## Acceptance Criteria
- [ ] `grep -rn "sah prompt\|prompt list\|prompt test" README.md doc/src docs/*.1` returns nothing.
- [ ] README no longer markets a prompts feature.
- [ ] `mdbook build` succeeds and `doc/book/searchindex.js` no longer indexes the removed prompt command.
- [ ] Agent/LLM "system prompt" wording is preserved where correct.

## Tests
- [ ] Add/keep a docs-lint or link-check step (if the repo has one) and run it; otherwise assert via `grep` in a CI check that `sah prompt` does not appear in `doc/src`.
- [ ] `cd doc && mdbook build` exits 0.

## Workflow
- Use `/tdd` for the grep-based guard (write the failing grep assertion / CI check first), then edit docs until it passes.