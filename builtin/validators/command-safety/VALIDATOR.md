---
name: command-safety
description: >-
  Flag dangerous shell patterns in scripts and commands in the diff. Look for
  destructive file operations, system damage, and download-and-execute pipes.
  Also look for credential exposure, unsafe git commands, and interactive
  editors. A confirmed dangerous command in the change is a blocker.
metadata:
  version: "{{version}}"
match:
  files:
    - "@file_groups/source_code"
---
