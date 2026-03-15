# Skills

Skills are the workflows of SwissArmyHammer. Each skill defines a specific development activity — planning, implementing, testing, reviewing, committing — as a self-contained unit that the AI agent can invoke.

## What a Skill Is

A skill is a markdown file with frontmatter metadata. When invoked (e.g., `/plan` or `/implement`), it expands into a full prompt that shapes the agent's behavior for that activity. Skills typically delegate heavy work to specialized subagents, keeping the parent conversation concise.

Skills are the primary interface between you and the SDLC system. You don't need to think about agents, tools, or validators directly — just invoke the skill and it orchestrates everything.

## Built-in Skills

SwissArmyHammer ships with skills covering the core development cycle:

| Skill | Purpose |
|-------|---------|
| **plan** | Research the codebase, decompose work into kanban cards |
| **implement** | Pick up one kanban card and implement it — write code, run tests |
| **implement-all** | Autonomously work through the entire kanban board |
| **test** | Run the test suite, analyze failures, fix issues |
| **review** | Structured code review with language-specific guidelines |
| **commit** | Clean, well-organized git commits |
| **coverage** | Analyze test coverage gaps on changed code |
| **deduplicate** | Find and refactor duplicate code |
| **double-check** | Verify recent work before proceeding |
| **code-context** | Explore codebase structure and symbol relationships |
| **shell** | Shell command execution with history and process management |
| **kanban** | Execute the next task from the kanban board |
| **lsp** | Diagnose and install language servers |

## How Skills Work

When you type `/implement`, here's what happens:

1. The skill definition is loaded and expanded into a prompt.
2. The prompt typically instructs the agent to delegate to a specialized **subagent** (the implementer agent mode).
3. The subagent does the work — reading code, writing files, running tests — using **tools**.
4. **Validators** check the subagent's work as it happens.
5. The subagent reports results back to the parent conversation.

This delegation pattern keeps verbose output (test results, implementation details) contained in the subagent, while the parent conversation gets a clean summary.

## Installing Skills

Beyond the built-in set, you can install additional skills from the Mirdan registry:

```bash
mirdan search "my-skill"
mirdan install my-skill
```

Or create your own by placing a `SKILL.md` file in the appropriate directory. Skills installed via Mirdan are deployed to each detected agent's skill directory automatically.

## Skill Locations

Skills are discovered from multiple locations with hierarchical precedence:

| Location | Scope |
|----------|-------|
| Built-in (embedded in binary) | Always available |
| Project `.claude/skills/` | Shared with team via git |
| User `~/.claude/skills/` | Personal, all projects |
| Installed via Mirdan | Project or global |
