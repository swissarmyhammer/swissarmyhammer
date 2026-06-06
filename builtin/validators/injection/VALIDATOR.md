---
name: injection
description: >-
  Flag unvalidated input flowing into SQL, shell commands, file paths, HTML, XML,
  or deserialization — SQL injection, command injection, path traversal, XSS,
  XXE, unsafe deserialization. A confirmed injection sink is a blocker.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
severity: error
---

# Injection Validator

Re-homed from the old multi-rule `security-rules` set (the `input-validation`
rule) into a focused, one-concern review-time validator: unvalidated input
reaching an injection sink. It is an **in-file judgment** — it reads the diff and
needs no engine probe, so it declares none.

This concern used to fire in real time, blocking a write before a vulnerable
sink hit disk. It is now a **review-time** validator: a confirmed injection
pattern stops work via the review-column gate (a blocker), not a pre-execution
block. The check reads the injection patterns already present in the changed
diff — narrower and after-the-fact — but a real injection vulnerability is still
a blocker.
