---
name: injection
description: >-
  Flag unvalidated input that flows into SQL, shell commands, file paths,
  HTML, XML, or deserialization code. Look for SQL injection, command
  injection, path traversal, XSS, XXE, and unsafe deserialization. A confirmed
  injection sink is a blocker.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
---
