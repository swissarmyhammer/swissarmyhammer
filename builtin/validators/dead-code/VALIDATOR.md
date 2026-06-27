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
---

# Dead Code Validator

An added symbol that nothing calls is dead weight that confuses every future
reader and hides intent. The engine runs the `callers` probe (`get callgraph`,
inbound) on each added symbol and attaches the inbound call sites as
ground-truth. An **empty inbound callgraph** on an added symbol that is not an
entry point, exported public API, or test is the dead-code signal — a fact,
delivered on the finding, that you confirm against the carve-outs before
reporting. A confirmed finding is a **blocker**.

One carve-out deserves emphasis: **forward-staged scaffolding**. In an
incremental, multi-step plan, a task routinely adds infrastructure — a field,
parameter, or helper — *ahead of* the task that consumes it, so its inbound
callgraph is legitimately empty until the follow-up lands. When the diff makes
that intent legible (a placeholder default a later change replaces, a value
plumbed through in preparation, or an explicit forward marker), it is
work-in-process, not dead code; do not block it. See the rule's carve-outs.
