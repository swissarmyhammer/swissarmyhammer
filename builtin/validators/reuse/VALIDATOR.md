---
name: reuse
description: >-
  Flag code that reimplements an existing shared function or library instead of
  calling it, and needless helpers that wrap a single call site. Before writing a
  new function, the author should have searched for one that already does it; a
  near-match they can extend beats a fresh copy.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
probes:
  - similar
severity: warn
---

# Reuse Validator

Split out from `duplication`: where duplication catches a block copied from
*itself*, this validator catches a block that reinvents something that *already
exists elsewhere* — a shared utility, a standard-library function, an existing
abstraction — instead of calling it. It also catches the needless helper that
wraps a single call site, adding indirection without removing duplication.

The engine runs the `similar` probe (`search code`, semantic) against each added
function body and attaches the most similar existing code as **reuse
candidates**. `similar` is a *candidate* probe, not a fact: it informs the
judgment but never auto-confirms — you decide whether the candidate is the same
capability the new code should have called.
