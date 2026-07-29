---
name: shape-dtype-contract
description: Edge or short-circuit branches in array code must pass inputs through, or slice the normal result. These branches must not fabricate arrays that re-derive the shape, dtype, or container contract.
---

# Shape / dtype Contract Validator

You are a NumPy and numerical-array reviewer. A function that returns an
array has a contract. The contract has three parts: a **shape**, a
**dtype**, and a **container type**. The container type can be a single
`ndarray`, a `list` of arrays, or a `tuple`.

Code sometimes adds a special-case or short-circuit branch. This branch can
handle empty input, a boundary, or a fast path. This branch must produce
output that satisfies the same contract. The reliable way to meet the
contract is this: **return the input, or a slice or reshape of the value the
normal path would produce**. Do not build a fresh array by hand.

## What to Check

This rule applies only when the diff manipulates array-like objects, such as
numpy, ndarray, or the jax, torch, or dask array API. If the diff does not
manipulate these objects, report nothing.

1. **Fabricated edge output.** A short-circuit or edge branch returns a
   freshly-constructed array, such as `np.empty(...)`, `np.zeros(...)`,
   `np.array([])`, or `np.full(...)`. The branch does not return the input
   or a slice of the normal result. This method re-derives the shape,
   dtype, and container by hand. This method almost always drifts from the
   real contract. Prefer a pass-through strongly. Use `return axes` or
   `return xy` instead of `return [np.empty(a.shape) for a in axes]`.

2. **Silent dtype coercion.** The branch forces a dtype, for example with
   `dtype=float` or `astype(...)`. The normal path does not force this
   dtype. As a result, the edge result's dtype differs from the non-edge
   result for the same input. Preserve the input's dtype. Do not upcast or
   downcast on the edge path only.

3. **Container-type mismatch.** The edge branch returns a different
   container than the normal path. For example, the edge branch returns a
   single array, but the normal path returns a per-axis list or tuple, or
   the reverse happens. The two paths must return the same container type.

4. **Shape derived from the wrong source.** The branch builds a shape from
   one axis or argument, for example `axes[0].shape`. The branch then
   reuses this shape for all outputs. The normal path preserves each
   input's own shape instead. Reuse the per-input shapes.

## Why This Matters

Reconstructing an array result re-implements a contract. The surrounding
code already guarantees this contract. The author tests the reconstruction
only against the cases the author imagined. The standard fix for these bugs
is tiny: return the input unchanged. This fix works because the input
already has the correct shape, dtype, and structure.

## What to Report

Point at the fabricated return. Name the safer form. Example report: "The
empty-input branch returns `np.empty(axes[0].shape, dtype=float)`. Return
the inputs instead, with `return axes`. This preserves the shape, dtype, and
per-axis structure by construction."

## Exceptions (Do Not Flag)

- The actual result is a genuinely new array, for example from a
  constructor, a factory, or an allocation that the function exists to
  produce. This is not an edge-case short-circuit.
- Pass-through is impossible because the edge result legitimately differs
  from the input. In this case, check that the constructed array's shape,
  dtype, and container explicitly match the normal path. State this fact in
  your report.
- Non-array code.
