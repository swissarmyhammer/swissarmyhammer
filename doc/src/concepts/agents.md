# Agents

Agents are specialized behavioral modes that shape how the AI approaches a task. When a skill needs to plan, implement, test, or review, it spawns a subagent with the appropriate mode — giving it a focused persona and set of instructions for that specific role.

## What an Agent Mode Is

An agent mode is a markdown document that defines a role. It tells the AI:

- What its job is (implement code, review changes, run tests)
- How to approach the work (test-driven development, layered review, minimal diffs)
- What patterns to follow (commit conventions, error handling, reporting format)
- What tools to prefer and how to use them

Agent modes aren't just system prompts — they're behavioral contracts that ensure consistent, high-quality output regardless of the specific task.

## Built-in Agent Modes

| Mode | Role | Spawned By |
|------|------|-----------|
| **planner** | Architecture and implementation planning | `/plan` |
| **implementer** | Code implementation with TDD practices | `/implement` |
| **tester** | Test execution and failure analysis | `/test` |
| **reviewer** | Structured code review with language guidelines | `/review` |
| **committer** | Clean git commit creation | `/commit` |
| **Explore** | Fast codebase exploration and discovery | `/code-context` |
| **Plan** | Plan mode for interactive planning | `/plan` (interactive) |
| **default** | General-purpose coding assistant | Fallback |
| **general-purpose** | Research and multi-step tasks | Ad-hoc delegation |

## The Subagent Pattern

Skills typically don't do work directly in the parent conversation. Instead, they delegate to a subagent:

```
Parent conversation
  │
  ├─ /implement
  │    └─ spawns implementer subagent
  │         ├─ reads kanban card
  │         ├─ writes code (using file tools)
  │         ├─ runs tests (using shell tool)
  │         ├─ validators check each change
  │         └─ reports results back to parent
  │
  └─ "Implementation complete, all tests passing"
```

This pattern has two key benefits:

1. **Context isolation.** Verbose output (test results, file contents, tool calls) stays inside the subagent. The parent conversation gets a clean summary.
2. **Focused behavior.** Each subagent operates with instructions optimized for its specific role, without the noise of unrelated context.

## How Modes Shape Behavior

Consider the difference between the **implementer** and **reviewer** modes:

The **implementer** is instructed to:
- Follow test-driven development (write failing test first, then make it pass)
- Make minimal changes — only what the kanban card requires
- Run tests after every change
- Report what was changed and whether tests pass

The **reviewer** is instructed to:
- Perform layered analysis: correctness first, then design, then style
- Apply language-specific review guidelines (Rust, TypeScript, Python, etc.)
- Capture findings as kanban cards for follow-up
- Never modify code directly — only report findings

Same tools, completely different behavior. The agent mode is what makes the difference.

## Language-Specific Guidelines

Some agent modes include language-specific guidelines that activate based on the code being worked with. The reviewer, for example, loads additional guidelines for:

- **Rust** — ownership patterns, error handling, unsafe usage
- **TypeScript/JavaScript** — type safety, async patterns, React conventions
- **Python** — type hints, exception handling, import organization
- **Go** — error handling, goroutine safety, interface design
- **Dart/Flutter** — widget patterns, state management

These guidelines are bundled as partials within the skill definition and selected automatically based on the files being reviewed.
