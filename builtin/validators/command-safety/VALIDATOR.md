---
name: command-safety
description: >-
  Flag dangerous shell patterns in scripts and commands embedded in the diff —
  destructive file operations, system damage, download-and-execute pipes,
  credential exposure, unsafe git, interactive editors. A confirmed dangerous
  command in the change is a blocker.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
severity: error
---

# Command Safety Validator

Re-homed from the old `command-safety` set into a focused, one-concern
review-time validator. It is an **in-file judgment** — it reads the diff and
needs no engine probe, so it declares none.

## Real-time → review-time: what changed

As a `PreToolUse` hook, this concern blocked a **proposed** command (e.g.
`rm -rf /`) *before execution* — it had the exact command the agent was about to
run and could refuse it.

There is no proposed command at review time. So this validator does something
narrower and after-the-fact: it reviews **shell scripts and commands embedded in
the diff** — `*.sh`/`*.bash`/`*.zsh` files, build/CI scripts, `Makefile` recipes,
and shell strings inside source (e.g. a `subprocess`/`exec` argument or a
`std::process::Command` invocation) — for the same dangerous patterns. It does
not, and cannot, gate a live command before it runs.

The same general note applies across the safety validators: confirmed dangerous
shell, secrets, or injection now stop work via the review-column gate (a
blocker), not a pre-execution block.
