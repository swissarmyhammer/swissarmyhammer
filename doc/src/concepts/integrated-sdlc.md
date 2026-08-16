# The Integrated SDLC

SwissArmyHammer assembles four kinds of building blocks — skills, agents, tools, and validators — into a coherent software development lifecycle. This page explains how they fit together.

## The Four Layers

```
┌─────────────────────────────────────────────┐
│                  Skills                      │
│   /plan  /implement  /test  /review /commit  │
│         (what to do — workflows)             │
├─────────────────────────────────────────────┤
│                  Agents                      │
│   planner  implementer  tester  reviewer     │
│         (how to think — personas)            │
├─────────────────────────────────────────────┤
│                  Tools                       │
│   files  shell  git  kanban  code-context    │
│         (what to work with — capabilities)   │
├─────────────────────────────────────────────┤
│                Validators                    │
│   code-hygiene  code-security  test-integrity│
│         (what's acceptable — guardrails)     │
└─────────────────────────────────────────────┘
```

Each layer has a distinct responsibility, and the layers compose vertically. A skill like `/implement` activates the **implementer** agent mode, which uses **tools** (file editing, shell execution, code context) to write code, and the **validators** whose globs match the changed files — `code-hygiene`, `code-security` and their siblings — then judge that code when the review pipeline runs.

## A Concrete Example

Here's what happens when you type `/plan` in your AI coding agent:

1. The **plan skill** activates. It defines the workflow: research the codebase, identify what needs to change, decompose work into discrete tasks.
2. The **planner agent** mode shapes how the AI thinks. It's instructed to be thorough in research, conservative in scope, and to produce kanban cards as output.
3. The planner uses **tools** to do the work: `code_context` to understand the codebase structure, `shell` to run analysis commands, `kanban` to create task cards, `question` to ask clarifying questions.
4. **Validators** do not run during planning. They match changed files, and planning changes none — the `code-security` set's `command-safety` rule reads the shell commands a diff embeds, not the commands the planner runs.

The result: a kanban board with well-scoped cards, ready for `/implement` to pick up.

## The Development Cycle

The skills form a natural development cycle:

### Plan → Implement → Test → Review → Commit

| Phase | Skill | Agent Mode | Primary Tools | Validators |
|-------|-------|------------|---------------|------------|
| **Plan** | `/plan` | planner | code-context, kanban, shell | none — planning changes no file |
| **Implement** | `/implement` | implementer | files, shell, code-context, kanban | code-hygiene, code-security |
| **Test** | `/test` | tester | shell, files | test-integrity, code-hygiene |
| **Review** | `/review` | reviewer | files, git, code-context | code-hygiene, code-security, completeness, duplication, reuse |
| **Commit** | `/commit` | committer | git, shell | none — committing changes no file |

Each phase is independent — you can run `/test` without `/plan`, or `/review` without `/implement`. But when used together, they form a complete cycle where each phase's output feeds the next.

### Supporting Skills

Beyond the core cycle, additional skills handle cross-cutting concerns:

- **`/coverage`** — analyzes test coverage gaps on changed code
- **`/deduplicate`** — finds and refactors duplicate code
- **`/double-check`** — validates recent work before moving on
- **`/implement-all`** — autonomously works through the entire kanban board
- **`/code-context`** — explores codebase structure and symbol relationships
- **`/shell`** — shell command execution with history and process management
- **`/lsp`** — diagnoses and installs language servers for code intelligence

## How the Pieces Connect

### Skills Activate Agent Modes

When a skill runs, it typically delegates to a specialized subagent. The `/implement` skill spawns an **implementer** agent, the `/review` skill spawns a **reviewer** agent, and so on. This keeps the parent conversation clean — verbose test output, detailed code analysis, and implementation details stay inside the subagent.

### Agent Modes Shape Behavior

Each agent mode is a markdown document that instructs the AI how to approach its task. The **implementer** follows test-driven development practices, writes minimal diffs, and reports results. The **reviewer** performs layered analysis (correctness, design, style) with language-specific guidelines. These aren't just prompts — they're behavioral contracts.

### Tools Provide Capabilities

Tools are MCP (Model Context Protocol) endpoints that the agent calls to interact with the outside world. File operations, shell execution, code intelligence, kanban management — these are all tools. Skills and agents don't hard-code tool usage; they decide which tools to use based on the task at hand.

### Validators Run Over the Changed Files

The review pipeline collects the changed files and runs each validator whose `match.files` globs match them. The `code-hygiene` set checks for commented-out code, overlong functions, and undocumented public APIs; the `code-security` set's `command-safety` rule reads the shell commands the diff embeds. Findings come back with each validator's severity, and blocking findings gate the change.

Per-tool-call dispatch is gone. Validators no longer run as Claude Code hooks on `PreToolUse`, `PostToolUse`, or `Stop`, and the legacy `trigger` key that named such an event has been removed from the rule schema.

## Extensibility

Every layer is extensible:

- **Skills** are markdown files. Drop a new one into `.claude/skills/` or install one via `mirdan install`.
- **Agent modes** are markdown files in the modes directory. Customize existing ones or create new specialized roles.
- **Tools** are MCP server endpoints. SwissArmyHammer's built-in tools cover the common cases; add more via MCP server configuration.
- **Validators** are AVP rule sets. Add project-specific rules under `./.validators/` or install shared ones via `mirdan install`.

The package manager (`mirdan`) ties extensibility to shareability — anything you create can be published to a registry and installed by others.
