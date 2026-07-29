---
name: double-check
description: Double check your work. Review changes, ask clarifying questions, and verify correctness before you proceed. Use this skill when the user says "double check", "verify", "sanity check", or wants validation of recent work.
license: MIT OR Apache-2.0
compatibility: This skill requires the `code_context` MCP tool for symbol lookup and blast-radius checks when it verifies recent work.
agent: double-check
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Double Check

Run an adversarial verification of recent work. Check its correctness, completeness, and alignment with intent.

## Process

1. **Hand off the change and its intent.** Give the `double-check` agent the recent changes, the related files, and the original intent or acceptance criteria. It uses the `code_context` MCP tool for symbol lookup and blast radius, and the `git` tool to `get changes`, to gather its own evidence.

2. **Let it verify adversarially.** The agent tries to prove the work is wrong, incomplete, or misaligned. It checks correctness (off-by-one errors, unhandled errors, missing edge cases, swapped arguments), completeness (acceptance criteria met, no TODOs, debug prints, commented-out code, or stubs), intent drift (scope not asked for, or asked-for scope missing), unverified "it works" claims, and broken blast-radius callers.

3. **Act on the returned verdict.** The agent returns a structured `VERDICT: PASS` or `VERDICT: REVISE`. Do not ask the user clarifying questions — act on the returned verdict.
   - **PASS** — the change is correct, complete, on-intent, and verified. Proceed.
   - **REVISE** — work through the severity-ranked findings. Apply each suggested fix to the change, then run double-check again until it passes.
