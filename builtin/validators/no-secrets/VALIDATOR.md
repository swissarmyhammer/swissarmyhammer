---
name: no-secrets
description: >-
  Flag secret-looking literals committed to code — API keys, access tokens,
  passwords, private keys, connection strings, webhook URLs with embedded
  secrets. A confirmed hardcoded credential is a blocker.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
severity: error
---

# No Secrets Validator

Re-homed from the old multi-rule `security-rules` set into a focused,
one-concern review-time validator: hardcoded secrets committed to code. It is an
**in-file judgment** — it reads the diff and needs no engine probe, so it
declares none.

This concern used to fire in real time, blocking a write before a secret hit
disk. It is now a **review-time** validator: a confirmed secret stops work via
the review-column gate (a blocker), not a pre-execution block. The check is
therefore after-the-fact — it reads the secret-looking literals already present
in the changed files — but the bar is unchanged: a real credential checked in is
a real leaked credential and must be removed.
