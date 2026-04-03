---
name: explore
description: Use this skill before planning or implementing when you need to understand code — how something works, why it behaves a certain way, or what a change would affect. Exploration is not done until you can articulate the test you would write. Use when the user says "explore", "investigate", "how does X work", "what would it take to change X", or when you need to understand code before acting.
metadata:
  author: "swissarmyhammer"
  version: "0.12.11"
---

# Explore

Understand code well enough to write the first failing test. Exploration without a testable conclusion is tourism.



## Why This Skill Exists

The gap between "I don't understand this code" and "I know what to build" is where most bad decisions happen. Claude's default behavior is to read files, grep around, and then jump to implementation. This skill enforces a structured path through that gap — using code-context as the primary tool and TDD as the exit criterion.

## The Rule

```
Exploration is complete when you can state:
  1. What test to write (assertion, not just test name)
  2. Where to put it (file path)
  3. Why it should fail right now (the gap between current and desired behavior)

If you can't state all three, you're not done exploring.
```

## Process

### 1. Orient — check the index

Always start here. If the index isn't ready, nothing else will be accurate.

```json
{"op": "get status"}
```

If TS indexed < 90%, wait and re-check. Don't explore with a stale index.

### 2. Survey — find the territory

Start broad. Use domain keywords from the user's question to find relevant symbols.

```json
{"op": "search symbol", "query": "<domain keyword>", "max_results": 15}
```

```json
{"op": "list symbols", "file_path": "<key file>"}
```

**What you're looking for**: the nouns and verbs of the problem. Structs, traits, functions that participate in the behavior you're investigating.

### 3. Trace — follow the execution

Once you've found the key symbols, trace how they connect.

```json
{"op": "get symbol", "query": "<specific symbol>"}
```

```json
{"op": "get callgraph", "symbol": "<symbol>", "direction": "both", "max_depth": 2}
```

**What you're looking for**: the path data takes through the system. Who calls what, what depends on what, where the boundaries are.

### 4. Scope — measure the blast radius

Before forming a hypothesis about what to change, understand what a change would touch.

```json
{"op": "get blastradius", "file_path": "<target file>", "max_hops": 3}
```

**What you're looking for**: how far a change propagates. If the blast radius surprises you, you don't understand the code well enough yet — go back to step 3.

### 5. Examine existing tests

Find how the code is already tested. This tells you what the project considers important, what patterns to follow, and where the gaps are.

```json
{"op": "grep code", "pattern": "<symbol or behavior under investigation>", "file_pattern": "test"}
```

Also use Glob/Grep to find test files near the code you're exploring:
- Same directory with `_test` suffix
- `tests/` directory at project or crate root
- Test modules inside source files (`#[cfg(test)]`, `describe(`, `#[test]`)

**What you're looking for**: existing test patterns to follow, assertions that already exist (so you don't duplicate), and gaps — behaviors that have no test coverage.

### 6. Conclude — state the test

This is the exit gate. Formulate your finding as a test specification:

```
TEST: <what to assert — the expected behavior>
FILE: <where the test goes — full path>
FAILS BECAUSE: <why this test doesn't pass today — the gap>
```

If you're exploring to understand (not to change), the conclusion is still test-shaped:

```
VERIFIED: <what the code does — the behavior you confirmed>
TESTED BY: <existing test that covers this, or "no existing test">
```

## Using code-context, not raw file reads

**code-context is the primary exploration tool.** It is faster, more accurate, and gives you structural information (symbols, call graphs, blast radius) that file reads cannot.

Use raw file reads (Read, Grep, Glob) only for:
- String literals, config values, error messages not in the symbol index
- Files that aren't code (TOML, YAML, JSON, Markdown)
- Confirming exact syntax when code-context gives you the location

**Do not** start exploration by reading files top-to-bottom. Start with `search symbol` and `get callgraph` to find the right code, then read only what you need.

## When to recurse

If the blast radius reveals unexpected dependencies, or the call graph leads to unfamiliar territory, loop back to step 2 with new keywords. Exploration is iterative — but each loop should narrow the focus, not widen it.

## When to escalate

If exploration reveals:
- **Work too large for a single test** — suggest `/plan` to break it into cards
- **A bug** — state the test that would catch it, suggest `/card` to track it
- **An architectural question** — present what you found and ask the user, don't guess

## Constraints

- **Don't write code during exploration.** Exploration produces understanding, not implementation. The test you articulate is a specification, not a file you create.
- **Don't skip steps.** Jumping from "search symbol" to "I know what to do" skips blast radius analysis — the step most likely to reveal surprises.
- **Don't explore forever.** If you've done 3 loops of steps 2-4 without converging on a test, stop and tell the user what's unclear. Ask for direction.
- **Don't use exploration to avoid acting.** Once you can state the test, exploration is done. Move to implementation or planning.
