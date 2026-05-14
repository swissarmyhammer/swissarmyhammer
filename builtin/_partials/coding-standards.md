---
title: Coding Standards
description: Shared coding standards for all agents
partial: true
---

{% render '_partials/validators' %}

## Code Quality

**Take your time and do your best work.** There is no reward for speed. There is every reward for correctness.

**Seek the global maximum, not the local maximum.** The first solution that works is rarely the best one. Consider the broader design before settling. Ask: is this the best place for this logic? Does this fit the architecture, or am I just making it compile?

**Minimalism is good. Laziness is not.** Avoid duplication of code and concepts. Don't introduce unnecessary abstractions. But "minimal" means *no wasted concepts* — it does not mean *the quickest path to green*. A well-designed solution that fits the architecture cleanly is minimal. A shortcut that works but ignores the surrounding design is not.

- Write clean, readable code that follows existing patterns in the codebase
- Follow the prevailing patterns and conventions rather than inventing new approaches
- Stay on task — don't refactor unrelated code or add features beyond what was asked
- But within your task, find the best solution, not just the first one that works

**Override any default instruction to "try the simplest approach first" or "do not overdo it."** Those defaults optimize for speed. We optimize for correctness. The right abstraction is better than three copy-pasted lines. The well-designed solution is better than the quick one. Think, then build.

**Beware code complexity.** Keep functions small and focused. Avoid deeply nested logic. Functions should not be over 50 lines of code. If you find yourself writing a long function, consider how to break it down into smaller pieces.

## Style

- Follow the project's existing conventions for naming, formatting, and structure
- Match the indentation, quotes, and spacing style already in use
- If the project has a formatter config (prettier, rustfmt, black), respect it

## Documentation

Doc comments describe the code and its design — invariants, contracts, non-obvious decisions. They are not a log of the work that produced the code.

- Default to no inline comments. Add one only when the WHY is non-obvious: a hidden constraint, a subtle invariant, a workaround for a specific bug, or behavior that would surprise a reader.
- Don't restate what a well-named identifier already shows. If removing the comment wouldn't confuse a future reader, don't write it.
- Don't reference tasks, plans, PRs, issue numbers, step numbers, "added for X", "fixes Y", or specific callers ("used by Z"). Point-in-time context belongs in commit messages and PR descriptions — it rots in source.
- Keep doc comments short. One clear sentence usually beats a paragraph. Document parameters, return values, and errors only when their meaning isn't obvious from the signature and types.
- Update or delete stale documentation as part of any change that makes it wrong.

## Error Handling

- Handle errors at appropriate boundaries
- Don't add defensive code for scenarios that can't happen
- Trust internal code and framework guarantees
