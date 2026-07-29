---
name: numpy
description: >-
  This validator reviews NumPy, ndarray, and other numerical array code. The
  code must preserve shape and dtype contracts. The code must pass inputs
  through. It must not fabricate new arrays. The code must handle empty or
  zero-size arrays. It must handle broadcasting. It must handle mixed or
  partial edges. The code must handle these cases for every calling
  convention the function supports. This validator applies only to diffs
  that manipulate array-like objects, such as numpy, ndarray, or the array
  API shared by jax, torch, or dask.
metadata:
  version: "{{version}}"
match:
  files:
    - "**/*.py"
---

# NumPy / Numerical-Array Review Validator

Numerical array code has contracts. Ordinary scalar code does not have these
contracts. A result has a shape, a dtype, and a container type. The container
type can be ndarray, list, or tuple. These contracts must hold across the
function's full input space. This input space includes empty or zero-size
arrays, scalars, broadcasting, and mixed inputs. In a mixed input, some axes
are empty and other axes are not empty. The hardest bugs are edge cases. The
author's own tests miss these edge cases. The tests miss the edge cases
because the author built the edge output by hand. The author did not derive
the edge output from the normal path.

This validator uses two in-file judgment rules. The rules read the diff. The
rules do not use an engine probe. Each rule fires only when the diff touches
array code. If the diff has no ndarray work, the rules report nothing.

- `shape-dtype-contract` — In an edge or short-circuit branch, return the
  input, or a slice of the normal result. Do not fabricate arrays with
  `np.empty`, `np.zeros`, or `np.array(...)`. A fabricated array re-derives
  the shape, dtype, and container contract by hand. A fabricated array can
  drift from the real contract.
- `array-edge-cases` — Handle empty or zero-size inputs, scalar inputs,
  broadcasting, and mixed or partial inputs. Guard the edge case at the same
  granularity and pipeline stage that the normal code uses. Test the edge
  case in every calling convention.

These rules are enforced rules. Each rule gives a binary pass or fail result.
These rules are not advisory. This validator is modeled on a real fix. The
real fix was a three-line pass-through. An elaborate, fabricated-output patch
failed to match the real fix.
