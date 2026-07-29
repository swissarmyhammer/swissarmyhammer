---
name: completeness
description: >-
  Find fixes that are correct in one place but incomplete overall. A fix may
  change the obvious site but not a sibling site. A fix may change a
  serializer but not its deserializer. A fix may stop an edge case from
  crashing without giving the right result. A fix may change a user-facing
  message or side effect without need, or drop it silently. These fixes pass
  the author's own tests. They fail the real test.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
    - "@file_groups/test_files"
---

# Completeness Validator

A fix can be correct in one place. The fix can still be incomplete overall.
The fix satisfies the one case the author had in mind. The author writes a
test for that exact case. The test passes. The change ships. But a symmetric
or sibling path stays broken. This validator reads the diff. It looks for
that gap.

This validator uses four rules. Each rule covers one concern. Each rule is an
**in-file judgment** over the diff. No engine probe is necessary.

- `inverse-operation-coverage` — This rule flags a change to one direction of
  a paired operation (write/read, encode/decode, serialize/deserialize,
  classify/parse) when the other direction has no test. It also flags a test
  that claims a round trip or symmetry but exercises only one direction.
- `invariant-propagation` — This rule flags a change to how a token, flag,
  format, or case is handled at one site. The change does not reach other
  sites that consume the same token.
- `public-output-contract` — This rule flags an existing user-facing message
  or output reformatted without need. It also flags an error that "goes
  away" because the diff swallows it silently, instead of keeping the
  intended side effect (warn, log, or return a value).
- `case-sensitivity-coverage` — This rule flags code that recognizes,
  parses, or dispatches on textual tokens when the tests use only one
  spelling. The case contract stays unproven in both directions: the
  positive direction (mixed case accepted) and the negative direction (wrong
  case rejected).

These four rules produce **warnings**, not blockers. Each warning marks a
place where a reviewer, or the implementer who picks up the task again, must
look harder before marking the work done. The lesson behind these rules is
simple: the author's own tests do not prove completeness.
