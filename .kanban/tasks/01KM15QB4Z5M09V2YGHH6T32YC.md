---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffcf80
title: Overhaul README with GSD-inspired structure
---
Rewrite the project README using lessons from get-shit-done's viral 34k-star README.

**Structure to follow:**
1. Pain-point headline (e.g. "AI that understands your codebase structure, not just your last message")
2. One-liner install front and center
3. Short personal "Why I Built This" narrative — anti-enterprise positioning, solo dev / small team energy
4. How It Works — visual workflow diagram (kanban → implement → test → review loop)
5. Command/skill reference table — clean, scannable
6. "Why It Works" section — context engineering via code-context, kanban state, structured skills
7. Configuration reference

**Key positioning: Multitool, not a pipeline.**
GSD locks you into a rigid sequence: discuss → plan → execute → verify → ship. Skip a step? Too bad. SAH gives you a kanban board and a set of sharp tools — you decide when to plan, implement, test, review, commit. Real work isn't linear. Sometimes you implement three cards then review them all at once. Sometimes you write tests first. Sometimes you skip review on a quick fix. A Swiss Army Hammer is a multitool: every tool is always available, you pick the right one for the moment.

**Skip for now:** social proof / testimonials (none yet), badges (add later)

**Key differentiators to highlight vs GSD:**
- Multitool philosophy vs rigid pipeline — more choices, not more ceremony
- Real kanban board (not flat markdown files)
- Treesitter-based code understanding (not just LLM-driven)
- MCP server architecture (typed APIs, not prompt injection)
- Test-driven workflow (TDD, test-loop, coverage)
- Structured code review
- Works across Claude Code, and any MCP-compatible host

**Tone:** Direct, opinionated, no enterprise theater. "Tools for people who ship."