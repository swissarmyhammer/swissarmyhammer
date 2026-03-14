# Quick Start

Get the integrated SDLC running in your project.

## Setup

```bash
# Install
brew install swissarmyhammer/tap/swissarmyhammer-cli

# Initialize in your project
cd your-project
sah init        # MCP server + tools + skills
avp init        # Validators (hooks)

# Verify
sah doctor
avp doctor
```

## The Development Cycle

Once initialized, the skills are available as slash commands in Claude Code:

### 1. Plan

```
/plan
```

Researches your codebase and creates kanban cards for the work ahead. The planner agent explores code structure, identifies dependencies, and decomposes the task into implementable units.

### 2. Implement

```
/implement
```

Picks up the next kanban card and implements it. The implementer agent writes code, runs tests, and reports results. Validators check every file write for code quality and security issues.

To work through the entire board autonomously:

```
/implement-all
```

### 3. Test

```
/test
```

Runs the test suite, analyzes failures, fixes issues, and reports back. The tester agent handles verbose test output so your conversation stays clean.

### 4. Review

```
/review
```

Performs a structured code review of your changes. The reviewer agent applies language-specific guidelines and captures findings.

### 5. Commit

```
/commit
```

Creates clean, well-organized git commits from your staged changes.

## Other Useful Skills

```
/coverage          # Find untested code in your changes
/deduplicate       # Find and refactor copy-paste code
/double-check      # Verify recent work before moving on
/code-context      # Explore codebase structure
/shell             # Run shell commands with history
```

## Installing More Skills

Browse and install community skills via Mirdan:

```bash
mirdan search "my-topic"
mirdan install some-skill
```

## Next Steps

- [The Integrated SDLC](../concepts/integrated-sdlc.md) — Understand how skills, agents, tools, and validators work together
- [Skills](../concepts/skills.md) — Deep dive into the skill system
- [Validators](../concepts/validators.md) — Configure quality guardrails
