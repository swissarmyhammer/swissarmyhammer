---
name: detected-projects
description: Discover project types, build commands, test commands, and language-specific guidelines for the current workspace. Use this skill when the user says "what kind of project", "detect project", "build command", "test command", "project type", asks what language or framework the code uses, or wants to know how to build, test, or format the project. Also use this skill early in any session, before you make changes.
license: MIT OR Apache-2.0
compatibility: This skill requires the `code_context` MCP tool. Project detection is implemented as the `code_context` `detect projects` operation.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Project Detection

Discover project types, build commands, and language guidelines for this workspace:

```json
{"op": "detect projects"}
```

**Call this early in your session**, before you make changes. The returned guidelines are authoritative for test, build, and formatting commands.
