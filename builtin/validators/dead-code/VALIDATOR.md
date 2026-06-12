---
name: dead-code
description: >-
  Flag any added or changed symbol with no inbound callers that is not an entry
  point, exported public API, or test. Also flag orphaned modules never wired
  into production, unreachable branches, and commented-out code. Dead code is a
  blocker: delete it, don't ship it.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
probes:
  - callers
severity: error
---

# Dead Code Validator

An added symbol that nothing calls is dead weight that confuses every future
reader and hides intent. The engine runs the `callers` probe (`get callgraph`,
inbound) on each added symbol and attaches the inbound call sites as
ground-truth. An **empty inbound callgraph** on an added symbol that is not an
entry point, exported public API, or test is the dead-code signal — a fact,
delivered on the finding, that you confirm against the carve-outs before
reporting. A confirmed finding is a **blocker**.
