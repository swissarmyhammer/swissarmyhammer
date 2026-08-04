---
name: code-security
description: >-
  Flag security defects in changed source code — hardcoded secrets and
  credentials, unvalidated input flowing into injection sinks (SQL, shell,
  path, HTML/XML, deserialization), and dangerous shell commands embedded in
  the diff. A confirmed defect is a blocker.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
---
