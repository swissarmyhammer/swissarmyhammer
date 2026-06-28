---
name: numpy
description: >-
  Reviewing NumPy / ndarray / numerical-array code: preserve shape and dtype
  contracts, prefer passing inputs through over fabricating new arrays, and
  handle empty/zero-size, broadcasting, and mixed/partial edges across every
  calling convention a function supports. Applies only to diffs that actually
  manipulate array-like (numpy/ndarray, or the array API shared by jax/torch/
  dask) objects.
metadata:
  version: "{{version}}"
match:
  files:
    - "**/*.py"
---

# NumPy / Numerical-Array Review Validator

Numerical array code has contracts that ordinary scalar code does not: a result
has a **shape**, a **dtype**, and a **container type** (ndarray vs list vs
tuple), and those must hold across the function's full input space — including
empty/zero-size arrays, scalars, broadcasting, and *mixed* inputs where some
axes are empty and others are not. The hard bugs here are edge cases that the
author's own tests miss because they reconstructed the edge output by hand
instead of deriving it from the normal path.

Two in-file judgment rules, read from the diff (no engine probe). Each fires
only when the diff genuinely touches array code; on a diff with no ndarray work
they report nothing.

- `shape-dtype-contract` — for an edge/short-circuit branch, return the input
  (or a slice of the normal result) rather than fabricating arrays
  (`np.empty`/`np.zeros`/`np.array(...)`) that re-derive the shape/dtype/
  container contract and drift from it.
- `array-edge-cases` — handle empty/zero-size, scalar, broadcasting and
  mixed/partial inputs; guard the edge at the same granularity and pipeline
  stage the normal code uses, and exercise it across every calling convention.

These are enforced rules (binary pass/fail), not advisory — the real fix this
validator is modelled on was a three-line pass-through that an elaborate,
fabricated-output patch failed to match.
