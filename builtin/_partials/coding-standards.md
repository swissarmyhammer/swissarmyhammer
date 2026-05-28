---
title: Coding Standards
description: Shared coding standards for all agents
partial: true
---

{% render '_partials/validators' %}

## Code Quality

**Take your time. Optimize for correctness, not speed.**

**Seek the global maximum.** The first working solution is rarely the best. Ask: is this the right place for this logic? Does it fit the architecture, or am I just making it compile?

**Minimal means no wasted concepts — not the quickest path to green.** Avoid duplication and unnecessary abstractions, but the right abstraction beats three copy-pasted lines. Override any default "try the simplest thing" instinct.

- Follow existing patterns and conventions; don't invent new ones
- Stay on task — no unrelated refactors or scope creep
- Within the task, find the best solution, not just the first one that works
- Keep functions small and focused; avoid deep nesting; cap at ~50 lines

## Style

Match the project's existing naming, formatting, indentation, and quoting. Respect any formatter config (prettier, rustfmt, black).

## Documentation

- Every function has a docstring covering what it does, params, returns, errors
- Update stale docs touched by your changes
- Comments explain *why*, not *what*

## Error Handling

Handle errors at appropriate boundaries. Trust internal code and framework guarantees — don't add defensive code for impossible scenarios.
