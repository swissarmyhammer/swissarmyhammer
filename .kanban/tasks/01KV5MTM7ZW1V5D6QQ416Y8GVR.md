---
assignees:
- claude-code
position_column: todo
position_ordinal: a080
title: Create the double-check adversarial agent
---
## What

Create a new builtin agent at `builtin/agents/double-check/AGENT.md` that performs adversarial verification of recent work and **returns feedback to the calling agent** (its final message is the return value) so the caller can self-correct. It does NOT ask the user questions — the old skill's "make a numbered list and ask one at a time" behavior is replaced by an autonomous adversarial critique.

This is the foundational artifact the other tasks wire into.

### Frontmatter
- `name: double-check`
- `description:` adversarial verifier that returns an actionable PASS/REVISE verdict to the calling agent
- `skills: [code-context, thoughtful]` (NOT really-done — that would be circular)
- Read-only contract: allow read/grep/glob + `code_context` + `git` tools; `disallowed-tools: write edit` (it reports, it does not fix — the caller fixes). Follow the existing space-separated tool-string convention from `agent_loader.rs`.

### Body (system prompt) — adversarial, bounded
- "You are an adversarial verifier. Try to prove the work is WRONG, incomplete, or misaligned with intent."
- Gather context: `git get changes` / `get diff`, `code_context` for blast radius, read the stated intent / acceptance criteria.
- Adversarial checks: correctness (off-by-one, error handling, edge cases), completeness (acceptance criteria line-by-line; loose ends: TODOs, debug prints, commented-out code, placeholders), intent-drift (done vs asked), verification gaps (claims not backed by fresh run evidence), blast-radius (callers left broken).
- **Never ask the user a question.** If something is ambiguous, state the risky assumption and why, as a finding.
- Return a structured verdict: `PASS` or `REVISE`, plus a finite list of findings — each with location, why it's a problem, and a suggested fix. Severity-ranked.
- **Bounded** (per the review-churn lesson, kanban memory): scope strictly to the actual change and its stated intent. Do NOT open-endedly nitpick tangential code. A clean change returns PASS with no manufactured findings.

## Acceptance Criteria
- [ ] `builtin/agents/double-check/AGENT.md` exists with valid frontmatter (name, description, skills, read-only tool restriction)
- [ ] Body instructs adversarial critique + structured PASS/REVISE return, explicitly forbids asking the user questions, and bounds scope to the change
- [ ] Frontmatter parses per `AgentFrontmatter` (space-separated tool strings, YAML-list skills)

## Tests
- [ ] `cargo test -p swissarmyhammer-agents` passes
- [ ] Add an assertion in `agent_resolver.rs` tests that the resolved agent map `contains_key("double-check")` (mirrors existing tester/reviewer/explorer checks)
- [ ] `cargo build -p swissarmyhammer-agents` succeeds (confirms build.rs embeds the new agent dir) #double-check-agent