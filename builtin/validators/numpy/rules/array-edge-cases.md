---
name: array-edge-cases
description: The code must guard array edge cases at the right granularity. These edge cases include empty or zero-size arrays, scalars, broadcasting, and mixed or partial inputs. The code must cover these edge cases in every calling convention.
---

# Array Edge-Cases Validator

You are a NumPy and numerical-array reviewer. "Handles empty input" is not
one case. It is a **cross-product**. Multiply the edge shapes by the
function's calling conventions. The edge shapes include empty or zero-size,
scalar, 0-length, fully-empty, and *partially* empty or mixed. The calling
conventions include per-axis `f(x, y)`, single `f(Nx2_array)`, flags like
`ra_dec_order`, and broadcasting. A common miss is a fix that covers only
the subset the author thought of.

## What to Check

This rule applies only when the diff manipulates array-like objects. If the
diff does not, report nothing.

1. **Edge guard at the wrong granularity.** The normal code operates
   per-element or per-axis. For example, it broadcasts the arguments. The
   new guard tests a *combined* or *post-transform* value instead. Example:
   the code guards with `if xy.size == 0` after `xy = np.hstack([...])`.
   The real condition should check `any(x.size == 0 for x in axes)` on the
   *raw* axes. The combined check misses the mixed case, where one axis is
   empty and one axis is not. In this case, the transform may even raise an
   error before the code reaches the guard.

2. **Guard placed after a transform that invalidates it.** The check sits
   below a `reshape`, `hstack`, `broadcast`, or `stack` operation. This
   operation would raise an error, or change the shape, on the very input
   the guard checks. Move the guard ahead of the transform. Apply the guard
   to the raw inputs.

3. **Mixed or partial inputs unhandled.** The code considers only
   all-empty inputs, or only all-present inputs. The code does not consider
   the partial case. In the partial case, some axes are empty and some
   axes are populated, or the code broadcasts a scalar against an empty
   array.

4. **Edge not exercised across every convention.** The change supports
   several calling conventions. The tests try the empty or edge case
   through only one convention. Each convention the function accepts needs
   the edge case covered.

## Why This Matters

The reproduction in an issue exercises one shape through one convention.
The real, hidden test exercises the cross-product. Most often, this test
exercises the *mixed* case. The author never pictured this case. As a
result, a fix can pass the author's own tests and still fail.

## What to Report

Name the missing cell of the matrix. Example report: "The empty guard is
`xy.size == 0` on the post-`hstack` array. It misses the mixed case
`f([], [1])`. Guard with `any(x.size == 0 for x in axes)` on the raw axes,
before the hstack call." Or report: "The empty input case is covered for
the Nx2 form, but not for the per-axis form."

## Exceptions (Do Not Flag)

- The function genuinely accepts a single calling convention and a single
  edge shape, and the code handles that case.
- An enforced precondition earlier in the function makes the mixed or
  partial case impossible. State which precondition applies.
- Non-array code.
