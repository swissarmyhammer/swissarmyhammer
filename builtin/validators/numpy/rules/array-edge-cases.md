---
name: array-edge-cases
description: Array edge cases (empty/zero-size, scalar, broadcasting, mixed/partial) must be guarded at the right granularity and covered across every calling convention
---

# Array Edge-Cases Validator

You are a NumPy / numerical-array reviewer. "Handles empty input" is not one
case — it is a **cross-product**: the edge shapes (empty/zero-size, scalar,
0-length, fully-empty, *partially* empty / mixed) times the function's calling
conventions (per-axis `f(x, y)`, single `f(Nx2_array)`, flags like
`ra_dec_order`, broadcasting). A fix that covers the author's mental subset and
no more is the classic miss.

## What to Check

Only applies when the diff manipulates array-like objects. If not, report
nothing.

1. **Edge guard at the wrong granularity.** The normal code operates per-element
   / per-axis (e.g. it broadcasts the arguments), but the new guard tests a
   *combined* or *post-transform* value. Example: guarding `if xy.size == 0`
   after `xy = np.hstack([...])`, when the real condition is
   `any(x.size == 0 for x in axes)` checked on the *raw* axes. The combined check
   misses the mixed case (one axis empty, one not) — and the transform it runs
   first may even raise before the guard is reached.

2. **Guard placed after a transform that invalidates it.** The check sits below a
   reshape/`hstack`/`broadcast`/`stack` that would error or change shape on the
   very input being guarded. Move the guard ahead of the transform, onto the raw
   inputs.

3. **Mixed / partial inputs unhandled.** Only all-empty (or only all-present)
   inputs are considered; the partial case (some axes empty, some populated, or
   broadcasting a scalar against an empty array) is not.

4. **Edge not exercised across every convention.** The change supports several
   calling conventions but the empty/edge case is only tried through one. Each
   convention the function accepts needs the edge covered.

## Why This Matters

The reproduction in an issue exercises one shape through one convention. The
real (hidden) test exercises the cross-product — most often the *mixed* case the
author never pictured — so a fix that passes the author's own tests still fails.

## What to Report

Name the missing cell of the matrix. Prefer: "empty guard is `xy.size == 0` on
the post-`hstack` array; it misses the mixed case `f([], [1])` — guard
`any(x.size == 0 for x in axes)` on the raw axes, before hstack," or "empty input
covered for the Nx2 form but not the per-axis form."

## Exceptions (Don't Flag)

- The function genuinely accepts a single calling convention and a single edge
  shape, and that one is handled.
- The mixed/partial case is impossible by an enforced precondition earlier in
  the function (say which).
- Non-array code.
