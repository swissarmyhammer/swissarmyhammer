---
name: double-check
description: Double check your work by reviewing changes, asking clarifying questions, and verifying correctness before proceeding. Use when the user says "double check", "verify", "sanity check", or wants validation of recent work.
metadata:
  author: swissarmyhammer
  version: 0.12.11
---

# Double Check

Review your recent work for correctness, completeness, and alignment with the user's intent.

## Process

### 1. Gather Context

- Review the current `git status` and recent changes using `git` with `op: "get changes"`
- Read the changed files in full to understand what was done
- Check any active kanban cards for the original requirements

### 2. Verify Correctness

For each changed file, check:

- **Does it compile/parse?** Run the appropriate build or lint command
- **Does it match the intent?** Compare what was done against what was asked
- **Are there obvious bugs?** Off-by-one errors, missing error handling, typos, wrong variable names
- **Are tests passing?** Run the test suite if tests exist for the changed code
- **Are there loose ends?** TODOs left behind, commented-out code, debug prints, placeholder values

### 3. Ask Clarifying Questions

If anything is unclear or ambiguous:

- Make a numbered list of questions
- Ask them **one at a time**, waiting for each answer before proceeding
- Questions should be specific and actionable, not vague

### 4. Report

Summarize what was checked and the result:

- What looks correct
- What issues were found (if any)
- What was unclear and needs the user's input
