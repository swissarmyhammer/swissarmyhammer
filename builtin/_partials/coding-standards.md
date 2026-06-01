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

## Reuse & Data-Driven Design

Left unchecked, generated code trends toward duplication and hardcoding. Push the other way by default.

- **Reuse before re-implementing.** Before writing a new function, search for one that already does it (`search symbol` / `grep code`). A near-match you can extend beats a fresh copy.
- **Extract before copy-pasting.** Two blocks that differ only by a value are one function with an argument. Don't paste-and-tweak.
- **Be data-driven.** Before hardcoding a value or enumerating cases in control flow, ask whether it's *data*. A `match`/`if`-chain over a known set whose arms differ only in constants is a table, not branching. Repeated literals are a named constant or config entry. Express variation as data (tables, maps, config, declarative specs) interpreted by a single code path — not as parallel code paths a human must keep in lockstep.
- **Calibrate, don't over-correct.** Warranted generalization removes *existing* duplication or serves a *real* variation axis. Rule of three: two occurrences is coincidence, three is a pattern. No second caller → no parameter. The right abstraction beats three copies; the wrong abstraction is worse than five.

## Style

Match the project's existing naming, formatting, indentation, and quoting. Respect any formatter config (prettier, rustfmt, black).

## Documentation

- Every function has a docstring covering what it does, params, returns, errors
- Update stale docs touched by your changes
- Comments explain *why*, not *what*

## Error Handling

Handle errors at appropriate boundaries. Trust internal code and framework guarantees — don't add defensive code for impossible scenarios.
