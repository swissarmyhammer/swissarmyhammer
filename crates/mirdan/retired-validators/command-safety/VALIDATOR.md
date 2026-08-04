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
---
