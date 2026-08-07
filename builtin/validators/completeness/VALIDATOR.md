---
name: completeness
description: >-
  Catch fixes that are correct-but-partial: a change applied at the obvious site
  but not its sibling sites, a serializer changed without its deserializer, an
  edge case made not-to-crash without producing the right result, or a
  user-facing message/side-effect needlessly changed or silently dropped. These
  are the fixes that pass the author's own tests yet fail the real one.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
    - "@file_groups/test_files"
probes:
  - inverse-pairs
  - public-surface
---

# Completeness Validator

A fix can be *locally* correct and still *globally* incomplete: it satisfies the
one case the author had in mind, the author writes a test for exactly that case,
the test passes, and the change ships — while a symmetric or sibling path the
same change implies is left broken. This validator reads the diff and looks for
that gap.

Four one-concern rules, each a judgment over the diff. Two read the diff alone.
Two read a probe as well: `inverse-operation-coverage` reads `inverse-pairs`,
which finds the paired names for it, and `public-output-contract` reads
`public-surface`, which computes what the change did to each file's
declarations:

- `inverse-operation-coverage` — a change to one direction of a paired operation
  (write/read, encode/decode, serialize/deserialize, classify/parse) without the
  other direction being exercised; or a test that *claims* round-trip/symmetry
  but only goes one way. The engine runs the `inverse-pairs` probe over each
  changed file and attaches one row per pair the change touched on one side
  only. `inverse-pairs` is a *candidate* probe: it finds the pairs, and the rule
  judges whether the partner needed the change.
- `invariant-propagation` — a change to how a token/flag/format/case is handled
  at one site, not applied at the other sites that consume the same token.
- `public-output-contract` — an existing user-facing message/output reformatted
  without need, an error made to "go away" by silently swallowing it instead
  of preserving the intended side-effect (warn / log / return value), or a
  declaration callers depend on removed, re-signed, or hidden. The engine runs
  the `public-surface` probe over each changed file and attaches one row per
  declaration the change added to, removed from, or re-spelled on the file's
  public surface. `public-surface` is a *fact* probe: it computes the surface
  change, and the rule judges whether the change broke a contract.
- `case-sensitivity-coverage` — code that recognizes/parses/dispatches on textual
  tokens whose tests only use one spelling, so the case contract is unproven in
  both the positive (mixed-case accepted) and negative (wrong-case rejected)
  direction.

These are **warnings**, not blockers: they mark places a reviewer (or the
implementer picking the task back up) must look harder before calling the work
done. The recurring lesson behind them: the author's own tests are not
sufficient evidence of completeness.
