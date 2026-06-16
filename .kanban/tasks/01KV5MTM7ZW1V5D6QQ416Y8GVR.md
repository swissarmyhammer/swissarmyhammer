---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffb080
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

## Review Findings (2026-06-15 07:53)

### Warnings
- [x] `crates/swissarmyhammer-agents/src/agent_resolver.rs:53` — Parameter type `PathBuf` is concrete; accept `impl AsRef<Path>` instead for better caller ergonomics. Callers currently must allocate or convert their `&str` or `&Path` to `PathBuf` unnecessarily. Change to `pub fn add_search_path(&mut self, path: impl AsRef<Path>)` and call `self.vfs.add_search_path(path.as_ref().to_path_buf(), FileSource::Local)` to convert inside the function.
- [x] `crates/swissarmyhammer-agents/src/agent_resolver.rs:191` — Literal "tester" appears 6 times across the tests module. Extract to a named constant to avoid duplication and reduce maintenance burden. Define const TESTER_AGENT: &str = "tester"; near the start of the tests module and use it throughout.

## Review Findings (2026-06-15 08:19)

### Warnings
- [x] `crates/swissarmyhammer-agents/src/agent_resolver.rs:116` — Function has 4-level deep nesting (for loop → if statement → match statement → match arms), exceeding the 3-level threshold. This makes the function harder to read and reason about. Extract the inner match logic into a helper function to reduce nesting. For example, create a `load_agent_or_warn` function that takes the path and source, performs the match, and handles both Ok/Err cases, then call it from inside the if block. This keeps the for-if-call chain at 3 levels.

## Resolution (2026-06-15 13:30)

Extracted the inner `match load_agent_from_dir(...)` out of `load_agents_from_directory` into a new `load_agent_or_warn(path, source, agents)` helper (with docstring), flattening the directory loop to the `for -> if -> call` 3-level chain. Behavior is identical. Verified: `cargo test -p swissarmyhammer-agents` => 109 passed; 0 failed. `cargo clippy -p swissarmyhammer-agents --all-targets` => 0 warnings (exit 0). All acceptance criteria, tests, and review findings verified.