---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffe780
project: skills-guide-review
title: Add user trigger phrases to `code-context` description
---
## What

Current description of `builtin/skills/code-context/SKILL.md` lists capabilities but has no **specific user trigger phrases**. The Anthropic guide (Chapter 2, "The description field") says descriptions MUST include both WHAT and WHEN, and the WHEN should mention specific phrases users would say.

Current: "Code context operations for symbol lookup, search, grep, call graph, and blast radius analysis. Use this skill before modifying code to understand structure, dependencies, and impact..."

There are no phrases like "blast radius", "who calls X", "find definition", "symbol lookup", "callgraph", "find references".

## Acceptance Criteria

- [x] Description adds specific user trigger phrases (e.g., "blast radius", "who calls", "find symbol", "callgraph", "find references").
- [x] Description keeps the concise WHAT, plus clear WHEN with trigger phrases.
- [x] Under 1024 chars, no `<`/`>`.

## Tests

- [x] Trigger test: "what calls this function?" → loads `code-context`.
- [x] Trigger test: "what's the blast radius of changing X?" → loads `code-context`.

## Reference

Anthropic guide, Chapter 2 — "The description field", examples of good descriptions with trigger phrases.

## Resolution

Updated `builtin/skills/code-context/SKILL.md` description (folded scalar preserved). New flattened length: 619 chars, no `<`/`>`. Added trigger phrases: "blast radius", "who calls this", "find symbol", "find references", "go to definition", "symbol lookup", "callgraph", "find callers", "what calls this function", "what's affected if I change this".

Regenerated `.skills/code-context/SKILL.md` by running `sah init project` (the project's standard skill-deploy path, invoked via the freshly-built `./target/debug/sah` so the new `builtin/skills/code-context/SKILL.md` description is baked in). Verified the deployed copy now contains all ten trigger phrases — `git diff .skills/code-context/SKILL.md` shows only the description line changing from the old text to the new one with trigger phrases. The runtime now loads the updated description end-to-end.

## Review Findings (2026-04-24 10:09)

### Warnings
- [x] `.skills/code-context/SKILL.md:3` — The generated `.skills/code-context/SKILL.md` is still the old pre-change description (no trigger phrases). Per MEMORY.md, `.skills/` is generated from `builtin/skills/` and must not be edited directly, but the generated copy IS checked into git (not gitignored) and is what the runtime loads. Until `.skills/` is regenerated and committed, the new trigger phrases in `builtin/skills/code-context/SKILL.md` will not actually reach runtime and the "Trigger test: 'what calls this function?' → loads `code-context`" acceptance check is not yet realized end-to-end. Fix: run the project's skill-regeneration step (the same step that produced commits like `262ed89c5 chore: skills`) so `.skills/code-context/SKILL.md` picks up the updated description, then commit. If regeneration is out of scope for this task, file a follow-up task for it and note the dependency here.
  - **Resolved (2026-04-24)**: Rebuilt `sah` from the current working tree (`cargo build --bin sah`) and ran `./target/debug/sah init project`. The regeneration updated `.skills/code-context/SKILL.md` (and other generated artifacts) from their `builtin/skills/` sources. `git diff .skills/code-context/SKILL.md` confirms only the description line changed, from the old copy without trigger phrases to the new one with all ten. Runtime-loaded description is now in sync with `builtin/`. #skills-guide