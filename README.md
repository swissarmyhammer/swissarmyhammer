<div align="center">

<img src="icon.png" alt="SwissArmyHammer" width="256" height="256">

# SwissArmyHammer

**A multitool for agent-driven engineering.**

**Every tool, any order. It's a multitool, not a pipeline.**

[![CI](https://github.com/swissarmyhammer/swissarmyhammer/workflows/CI/badge.svg)](https://github.com/swissarmyhammer/swissarmyhammer/actions)
[![License](https://img.shields.io/badge/License-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.91+-orange.svg)](https://www.rust-lang.org/)
[![MCP](https://img.shields.io/badge/MCP-compatible-green.svg)](https://github.com/anthropics/model-context-protocol)

</div>

---

AI coding agents are powerful -- but without structure, they wander. They lose track of the plan. They skip tests. They write code that works but isn't reviewed. They forget what they were doing halfway through.

Other tools try to fix this by locking you into a rigid pipeline: discuss, then plan, then execute, then verify, then ship. In that order. Skip a step? Too bad.

Real work isn't a pipeline. Sometimes you implement three things, then review them all at once. Sometimes you write tests first. Sometimes you skip review on a quick fix and just ship it.

SwissArmyHammer gives you a kanban board and a set of sharp tools. You decide when to plan, implement, test, review, and commit. Every tool is always available. You pick the right one for the moment. No sprint ceremonies. No story points. No enterprise theater. Just tools for people who ship.

```
  ┌──────────┐
  │  /plan   │  Break work into kanban cards
  └────┬─────┘
       │         ┌──────────────────────────┐
       ▼         │  Any tool, any time,     │
  ┌──────────┐   │  any order.              │
  │/implement│   │                          │
  └────┬─────┘   │  /test after /implement? │
       │         │  /review before /commit? │
       ▼         │  /implement three cards  │
  ┌──────────┐   │  then /review them all?  │
  │  /test   │   │                          │
  └────┬─────┘   │  Your call.              │
       │         └──────────────────────────┘
       ▼
  ┌──────────┐     ┌──────────┐
  │ /review  │────▶│ /commit  │
  └──────────┘     └──────────┘
```

Works with Claude Code, Cursor, Windsurf, or any MCP-compatible agent.

## Get Started in 30 Seconds

Get in your project directory and run:

```bash
cd <your project directory>
brew install swissarmyhammer/tap/swissarmyhammer-cli
sah init
```

That's it. Your agent now has skills, tools, and workflows. 

## Two Ways to Plan

The `/plan` skill is the front door to SwissArmyHammer. It works two ways:

### Hand it a spec

Write your requirements in a markdown file and point the agent at it:

```
> /plan my-feature-spec.md
```

The agent reads your spec, explores the codebase to understand what exists, then creates a kanban board with ordered tasks, subtasks, acceptance criteria, and test requirements. Each card has enough context that the agent (or a teammate) can pick it up and implement it without re-reading the spec.

Your spec can be as simple or detailed as you want -- a few bullet points, a full PRD, or anything in between. The agent fills in the implementation details by reading your actual code.

### Or just talk it through

You don't need a spec file. Start a conversation and plan interactively:

```
> I want to add OAuth2 support to the API
```

The agent enters planning mode, asks clarifying questions, explores your codebase, and builds the kanban board incrementally as you discuss. You can steer the plan in real time:

```
> Split that auth card into separate cards for Google and GitHub providers
> Add a card for the token refresh flow -- we'll need that too
> Actually, let's do GitHub first and Google in a follow-up PR
```

The plan evolves through conversation. Cards get added, split, merged, and reordered based on your feedback. When you're happy, say "go" and the agent starts implementing.

### Then execute

Either way, once the plan is on the board:

```
> /implement                     # Do one kanban card at a time, with context-aware code editing and testing
> /implement-loop                # RalphLoop those kanban cards. Go for a walk with your 🦮.
> /test                          # Run tests, report failures as cards
> /test-loop                     # RalphLoop test→fix→test until green
> /review                        # Code review -- findings become new cards
> /commit                        # Clean conventional commit
```

## What You Get

### The Problem With Other Approaches

Pipeline tools force a rigid sequence: discuss, plan, execute, verify, ship. Every task goes through the same ceremony, whether it's a three-month rewrite or a one-line fix. You can't skip steps, can't reorder them, can't adapt to how the work actually flows.

SwissArmyHammer is different. Every stage of the software development lifecycle is an independent tool. Use what you need, skip what you don't. The kanban board is the shared state -- not a sequential pipeline.

| Stage | What happens | Command |
|-------|-------------|---------|
| **Plan** | Read your spec, explore the codebase, create a kanban board with ordered tasks | `/plan` |
| **Implement** | Pick up tasks one-by-one, write code, run tests, mark complete | `/implement` |
| **Test** | Run the full suite, report failures as kanban cards | `/test` |
| **Coverage** | Find untested code, create cards for the gaps | `/coverage` |
| **Review** | Structured code review -- findings become kanban cards | `/review` |
| **Commit** | Stage changes, write a conventional commit message | `/commit` |
| **Explore** | Semantic code search and symbol lookup across 25+ languages | `/code-context` |
| **Deduplicate** | Find near-duplicate code and refactor it | `/deduplicate` |
| **Shell** | Execute commands with persistent, searchable output history | `/shell` |
| **Double-check** | Verify recent work before moving on | `/double-check` |
| **LSP** | Diagnose and install missing language servers | `/lsp` |

These tools connect into loops. `/plan` creates cards. `/implement` works through them. `/review` finds issues and creates new cards. `/implement` picks those up too. `/coverage` finds untested code and creates test cards. It's a closed loop -- but you control the order.

## Context Management -- The Hidden Superpower

The biggest bottleneck for AI agents isn't intelligence -- it's context. Long test output blows the context window. The agent can't find the function it needs. It re-reads files it already scanned. SwissArmyHammer solves this at every level.

### Smart Shell (not just `bash -c`)

The built-in shell isn't a thin wrapper around subprocess exec. It's a **virtual shell with persistent history, process management, and searchable output**:

- **Every command's output is stored and indexed** -- even if the response was truncated to save tokens
- **Semantic search across all output** -- ask "find the authentication error" and it matches "403 forbidden" and "login denied"
- **Regex grep across history** -- `error\[E\d+\]` finds every Rust compiler error from every command you've run
- **Line-range retrieval** -- output was truncated? Fetch lines 450-500 of command #3 without re-running it
- **Configurable output limits** -- return 50 lines, 200, or zero (fire-and-forget). Full output is always saved for later

This means your agent can run `cargo test` with 10,000 lines of output, get a 50-line summary, and then surgically search for the failure -- without burning context tokens on scrollback.

### Automatic Code Intelligence (tree-sitter + LSP)

SwissArmyHammer automatically indexes your codebase using tree-sitter and LSP. No configuration, no manual setup -- open a project and it starts parsing in the background.

- **Symbol lookup** -- jump to any definition with fuzzy matching (`MyStruct::new`, `process_req`, partial names)
- **Call graph traversal** -- who calls this function? What does it call? Trace execution flow across files
- **Blast radius analysis** -- before you change `validate_token`, see every file and function transitively affected
- **Semantic diffs** -- `git diff` shows line changes; sah shows entity-level changes (Added, Modified, Deleted, Moved, Renamed)
- **25+ languages** -- Rust, Python, TypeScript, Go, Java, C/C++, Ruby, Swift, Kotlin, and more

This is what lets `/plan` actually understand your codebase before creating tasks, and what lets `/review` catch real architectural issues instead of just style nits.

## The Suite

SwissArmyHammer is three tools that work together:

### [sah](swissarmyhammer-cli/) -- Skills and Tools for Any Agent

The core. An MCP server that gives your agent everything it needs:

**Tools** -- the building blocks:
| Tool | What it does |
|------|-------------|
| **Files** | Read, write, edit, glob, grep -- with .gitignore support |
| **Git** | Branch, commit, diff, status, PR workflows |
| **Shell** | Safe command execution with security hardening |
| **Kanban** | File-backed task boards -- cards, subtasks, dependencies, tags |
| **Code Search** | Tree-sitter powered semantic search across 25+ languages |
| **Web** | Fetch pages and convert to markdown, search the web |
| **JavaScript** | Embedded QuickJS expression evaluation |
| **Questions** | Elicitation-based Q&A for capturing decisions |

**Skills** -- the workflows that use those tools:

Skills are markdown files. They teach your agent *how* to do things, not just *what* to do. Each skill defines a step-by-step process, and a specialized agent type executes it. This is what turns a generic LLM into a focused engineer.

You can write your own skills too -- drop a `SKILL.md` in `.sah/skills/my-skill/` and your agent picks it up automatically.

### [avp](https://agentvalidatorprotocol.com) -- Agent Validator Protocol

Guardrails. `avp` hooks into your agent as a pre/post validator. Every time the agent writes or edits a file, validators run automatically and **block bad changes before they land**.

- **code-quality** -- cognitive complexity, function length, naming, magic numbers, dead code
- **security-rules** -- no secrets in code, input validation
- **command-safety** -- safe shell command checks
- **test-integrity** -- catches test cheating (skipping tests, weakening assertions, hardcoding expected values)

```bash
avp init        # Install validators into your project
avp list        # See what's active
```

### [mirdan](https://mirdan.ai) -- Package Manager for Agent Skills

npm for AI agents. Install skills and validators from a shared registry. Publish your own.

```bash
mirdan search code-quality     # Find packages
mirdan install code-quality    # Install one
mirdan agents                  # See your agents
mirdan publish                 # Share your own
```

## Architecture

**Everything is markdown.** Skills, validators, workflows, prompts -- all markdown with YAML frontmatter and Liquid templating. No proprietary formats, no databases, no cloud lock-in. Everything lives in your repo or your home directory, fully version-controllable.

```
~/.sah/
  skills/           # Installed skills (markdown)
  validators/       # Installed validators (markdown)
  prompts/          # Custom prompts (markdown + Liquid)
  workflows/        # State machine workflows (markdown + Mermaid)
```

Project-level overrides go in `.sah/` in your repo. Project settings win over user settings.

The MCP server itself is a single Rust binary -- fast startup, no runtime dependencies, no Docker, no cloud services. It runs locally alongside your agent.

## Why SwissArmyHammer?

**For individual developers:** Your agent becomes dramatically more capable. Instead of babysitting it through each step, you hand it a spec and walk away. It plans, implements, tests, reviews, and commits -- following the same engineering process you would.

**For teams:** Consistent engineering process across every developer's agent. The same skills, the same validators, the same quality gates. Install once via `mirdan`, and every team member's agent works the same way.

**For tool builders:** The skill/validator format is just markdown. Build a skill in minutes, share it with `mirdan publish`, and it works in any MCP-compatible agent. No SDK to learn, no API to integrate.

## License

MIT OR Apache-2.0
