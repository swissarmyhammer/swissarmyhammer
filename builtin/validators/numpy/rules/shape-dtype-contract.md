---
name: shape-dtype-contract
description: Edge/short-circuit branches in array code must pass inputs through or slice the normal result, not fabricate arrays that re-derive the shape/dtype/container contract
severity: error
---

# Shape / dtype Contract Validator

You are a NumPy / numerical-array reviewer. A function that returns an array has
a contract: a **shape**, a **dtype**, and a **container type** (a single
`ndarray`, a `list` of arrays, a `tuple`, ...). When code adds a special-case or
short-circuit branch — for empty input, a boundary, a fast path — the branch
must produce output that satisfies that *same* contract. The reliable way to do
that is to **return the input, or a slice/reshape of the value the normal path
would produce** — not to build a fresh array by hand.

## What to Check

Only applies when the diff manipulates array-like (numpy/ndarray, or jax/torch/
dask array-API) objects. If it doesn't, report nothing.

1. **Fabricated edge output.** A short-circuit/edge branch returns a
   freshly-constructed array — `np.empty(...)`, `np.zeros(...)`, `np.array([])`,
   `np.full(...)` — instead of returning the input or a slice of the normal
   result. This re-derives the shape/dtype/container by hand and almost always
   drifts from the real contract. Strongly prefer pass-through:
   `return axes` / `return xy` over `return [np.empty(a.shape) for a in axes]`.

2. **Silent dtype coercion.** The branch forces a dtype (`dtype=float`,
   `astype(...)`) that the normal path would not, so the edge result's dtype
   differs from the non-edge result for the same input. Preserve the input's
   dtype; don't upcast/downcast on the edge path only.

3. **Container-type mismatch.** The edge branch returns a different container
   than the normal path — a single array where the normal path returns a
   per-axis list/tuple, or vice versa. The two paths must agree.

4. **Shape derived from the wrong source.** The branch builds a shape from one
   axis/argument (`axes[0].shape`) and reuses it for all outputs, where the
   normal path preserves each input's own shape. Reuse the per-input shapes.

## Why This Matters

Reconstructing an array result re-implements a contract the surrounding code
already guarantees, and the reconstruction is tested only against the cases the
author imagined. The canonical fix for these bugs is tiny — return the input
unchanged — precisely because the input already has the correct shape, dtype,
and structure.

## What to Report

Point at the fabricated return and name the safer form. Prefer: "empty-input
branch returns `np.empty(axes[0].shape, dtype=float)`; return the inputs
(`return axes`) so shape, dtype, and per-axis structure are preserved by
construction."

## Exceptions (Don't Flag)

- A genuinely new array is the actual result (a constructor/factory, an
  allocation the function exists to produce) — not an edge-case short-circuit.
- Pass-through is impossible because the edge result legitimately differs from
  the input, and the constructed array's shape/dtype/container is explicitly
  matched to the normal path (say so).
- Non-array code.
