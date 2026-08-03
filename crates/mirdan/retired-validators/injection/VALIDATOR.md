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
---
