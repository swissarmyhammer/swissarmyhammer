---
name: double-check
description: Double check your work by reviewing changes, asking clarifying questions, and verifying correctness before proceeding. Use when the user says "double check", "verify", "sanity check", or wants validation of recent work.
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool for symbol lookup and blast-radius checks used when verifying recent work.
agent: double-check
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Double Check

Run an adversarial verification of recent work for correctness, completeness, and alignment with intent.

## Process

1. **Hand off the change and its intent.** Give the `double-check` agent the recent changes, the related files, and the original intent / acceptance criteria. It uses the `code_context` MCP tool for symbol lookup and blast radius, and the `git` tool to `get changes`, to gather its own evidence.

2. **Let it verify adversarially.** The agent tries to prove the work is wrong, incomplete, or misaligned: correctness (off-by-one, unhandled errors, missing edge cases, swapped arguments), completeness (acceptance criteria satisfied, no TODOs / debug prints / commented-out code / stubs), intent drift (scope not asked for, asked-for scope missing), unverified "it works" claims, and broken blast-radius callers.

3. **Act on the returned verdict.** The agent returns a structured `VERDICT: PASS` or `VERDICT: REVISE`. Do not ask the user clarifying questions — the verdict is the return value to act on.
   - **PASS** — the change is correct, complete, on-intent, and verified. Proceed.
   - **REVISE** — work through the severity-ranked findings, applying each suggested fix to the change, then double-check again until it passes.
