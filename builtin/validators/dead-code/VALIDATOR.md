---
name: dead-code
description: >-
  Flag any added or changed symbol with no inbound callers, unless it is an
  entry point, an exported public API, or a test. Also flag orphaned modules
  that are never wired into production, unreachable branches, and
  commented-out code. Dead code is a blocker. Delete it. Do not ship it.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
probes:
  - callers
---

# Dead Code Validator

An added symbol that nothing calls is dead weight. It confuses every future
reader and hides the intent of the code. The engine runs the `callers` probe
(`get callgraph`, inbound) on each added symbol. It attaches the inbound
call sites as ground truth. An **empty inbound callgraph** on an added
symbol is the dead-code signal, unless the symbol is an entry point, an
exported public API, or a test. This is a fact on the finding. Confirm it
against the carve-outs before you report it. A confirmed finding is a
**blocker**.

One carve-out needs emphasis: **forward-staged scaffolding**. In an
incremental, multi-step plan, a task often adds infrastructure — a field, a
parameter, or a helper — ahead of the task that consumes it. Its inbound
callgraph is empty on purpose, until the follow-up task lands. The diff can
make this intent clear in one of these ways: a placeholder default that a
later change replaces, a value passed through in preparation, or an explicit
forward marker. When the diff shows this intent, the code is
work-in-process, not dead code. Do not block it. See the rule's carve-outs.
