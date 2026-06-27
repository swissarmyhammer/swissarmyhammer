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
---

# Completeness Validator

A fix can be *locally* correct and still *globally* incomplete: it satisfies the
one case the author had in mind, the author writes a test for exactly that case,
the test passes, and the change ships — while a symmetric or sibling path the
same change implies is left broken. This validator reads the diff and looks for
that gap.

Three one-concern rules, each an **in-file judgment** over the diff (no engine
probe required):

- `inverse-operation-coverage` — a change to one direction of a paired operation
  (write/read, encode/decode, serialize/deserialize, classify/parse) without the
  other direction being exercised; or a test that *claims* round-trip/symmetry
  but only goes one way.
- `invariant-propagation` — a change to how a token/flag/format/case is handled
  at one site, not applied at the other sites that consume the same token.
- `public-output-contract` — an existing user-facing message/output reformatted
  without need, or an error made to "go away" by silently swallowing it instead
  of preserving the intended side-effect (warn / log / return value).

These are **warnings**, not blockers: they mark places a reviewer (or the
implementer picking the task back up) must look harder before calling the work
done. The recurring lesson behind them: the author's own tests are not
sufficient evidence of completeness.
