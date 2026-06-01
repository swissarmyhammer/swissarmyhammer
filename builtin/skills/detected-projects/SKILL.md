---
name: detected-projects
description: Discover project types, build commands, test commands, and language-specific guidelines for the current workspace. Use when the user says "what kind of project", "detect project", "build command", "test command", "project type", asks what language or framework the code uses, or wants to know how to build, test, or format the project. Also use early in any session before making changes.
license: MIT OR Apache-2.0
compatibility: Requires the `code_context` MCP tool; project detection is implemented as `code_context` `detect projects`.
metadata:
  author: swissarmyhammer
  version: "{{version}}"
---

# Project Detection

Discover project types, build commands, and language guidelines for this workspace:

```json
{"op": "detect projects"}
```

**Call early in your session**, before changes. The returned guidelines are authoritative for test, build, and formatting commands.
