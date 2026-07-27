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
---
