---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffb180
title: Add ClaudeMd initializable + doctor check to ensure CLAUDE.md loads thoughtful skill
---
## What

Add a new `Initializable` component (`ClaudeMd`) and a corresponding `sah doctor` check that ensures a project's `CLAUDE.md` exists and begins with an instruction to load the thoughtful skill.

### Files to create/modify

- **`swissarmyhammer-cli/src/commands/install/components/claude_md.rs`** — New file. Implement `Initializable` for a `ClaudeMd` struct.
  - `name()` → `"claude-md"`
  - `category()` → `InitCategory::Project`
  - `priority()` → `22` (after `ProjectStructure` at 20, before `KanbanTool` at 25)
  - `is_applicable()` → true for `Project` and `Local` scopes
  - `init()` — If `CLAUDE.md` does not exist at the git root, create it with the mandatory preamble. If it exists but does not start with the preamble, prepend it. Report actions via `InitEvent::Action`.
  - `deinit()` — No-op (don't delete user's CLAUDE.md)

- **`swissarmyhammer-cli/src/commands/install/components/mod.rs`** — Add `mod claude_md;` and register `ClaudeMd` in `register_all()`.

- **`swissarmyhammer-cli/src/commands/doctor/checks.rs`** — Add `check_claude_md()` function that verifies CLAUDE.md exists and starts with the mandatory preamble. Return `HealthCheck` with `Status::Healthy` or `Status::Fixable` (with fix hint: "run `sah init`").

- **`swissarmyhammer-cli/src/commands/doctor/mod.rs`** — Call `check_claude_md()` from the doctor run.

### Preamble content

The mandatory preamble is:
```
MANDATORY: Before responding to ANY prompt (including clarifying questions), invoke the Skill tool with skill: "thoughtful" first.
```

The check should verify the first non-empty line of CLAUDE.md contains this text. If CLAUDE.md has other content below the preamble, preserve it.

## Acceptance Criteria

- [ ] `sah init` in a project without CLAUDE.md creates one with the thoughtful preamble
- [ ] `sah init` in a project with an existing CLAUDE.md that lacks the preamble prepends it (preserving existing content)
- [ ] `sah init` in a project with CLAUDE.md already containing the preamble is a no-op
- [ ] `sah doctor` reports healthy when CLAUDE.md has the preamble
- [ ] `sah doctor` reports fixable when CLAUDE.md is missing or lacks the preamble
- [ ] `sah deinit` does NOT delete or modify CLAUDE.md

## Tests

- [ ] Unit test in `claude_md.rs`: init creates CLAUDE.md with preamble when file is absent (use tempdir)
- [ ] Unit test in `claude_md.rs`: init prepends preamble to existing CLAUDE.md content
- [ ] Unit test in `claude_md.rs`: init is idempotent — second run does not duplicate preamble
- [ ] Unit test in `checks.rs`: `check_claude_md()` returns healthy for valid CLAUDE.md
- [ ] Unit test in `checks.rs`: `check_claude_md()` returns fixable for missing CLAUDE.md
- [ ] Unit test in `checks.rs`: `check_claude_md()` returns fixable for CLAUDE.md without preamble
- [ ] `cargo nextest run` passes with no regressions