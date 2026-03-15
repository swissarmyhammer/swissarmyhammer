# SwissArmyHammer

SwissArmyHammer is an integrated software development lifecycle (SDLC) platform for AI-powered coding agents. It combines **skills**, **agents**, **tools**, and **validators** into a single system that turns your AI coding assistant into a complete development team.

Instead of a loose collection of scripts and prompts, SwissArmyHammer provides a structured composition of capabilities — each with a clear role — that work together to plan, implement, test, review, and ship code.

## Three CLIs, One System

SwissArmyHammer ships as three complementary command-line tools:

| CLI | Role |
|-----|------|
| **`sah`** | The core engine. MCP server, skills, tools, agents, and workflows. |
| **`avp`** | The validator. Hook-based code quality enforcement that runs alongside your agent. |
| **`mirdan`** | The package manager. Install, publish, and share skills, validators, tools, and plugins across agents and teams. |

Each tool is independently useful, but they're designed to work together. `sah` provides the capabilities, `avp` enforces the guardrails, and `mirdan` lets you share and reuse everything.

## How It Works

At its core, SwissArmyHammer extends AI coding agents (like Claude Code) with a composable set of SDLC primitives:

1. **Skills** define *what to do* — plan, implement, test, review, commit. Each skill is a self-contained workflow that the agent can invoke.
2. **Agents** (subagent modes) define *how to think* — a planner reasons differently than a tester. Agent modes shape the AI's behavior for specific roles.
3. **Tools** provide *what to work with* — file operations, shell execution, code intelligence, kanban boards, git integration. These are the hands of the system.
4. **Validators** enforce *what's acceptable* — code quality rules, security checks, test integrity. These run as hooks, catching problems before they land.

The agent orchestrates these pieces through a natural conversation interface. Say `/plan` and the system researches your codebase, decomposes work into kanban cards, and presents a plan. Say `/implement` and it picks up the next card, writes code, runs tests, and reports back. Say `/review` and a dedicated reviewer agent examines the changes with language-specific guidelines.

## The SDLC Loop

A typical development cycle with SwissArmyHammer looks like:

```
/plan  →  /implement  →  /test  →  /review  →  /commit
  ↑                                                |
  └────────────────────────────────────────────────┘
```

Each step is a skill backed by specialized agent modes and tools. The kanban board tracks progress across steps. Validators run continuously in the background, enforcing quality at every stage.

This isn't a rigid pipeline — you can use any skill independently, skip steps, or run them in any order. The system is designed to support how developers actually work, not to impose a process.

## What Makes It Different

- **Composition over configuration.** Skills, agents, tools, and validators are separate, pluggable units. Mix and match what you need.
- **Agent-native.** Built from the ground up as an MCP server for AI coding agents, not retrofitted from human-oriented tooling.
- **Quality built in.** Validators run as hooks on every tool call — code quality, security, and test integrity are enforced automatically, not as an afterthought.
- **Shareable.** Mirdan provides a package registry so teams can publish and install skills, validators, tools, and plugins. Your team's best practices become installable packages.
- **Language-aware.** Review guidelines, coverage analysis, and code intelligence adapt to the language you're working in — Rust, TypeScript, Python, Go, and more.
