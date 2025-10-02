---
title: Consistent Naming Conventions
description: Enforce consistent naming conventions for the language
category: code-quality
severity: info
tags: ["code-quality", "style"]
---

Check {{ language }} code for consistent naming conventions according to language standards.

Apply language-specific conventions:

**Rust:**
- snake_case for functions, variables, modules
- PascalCase for types, structs, enums, traits
- SCREAMING_SNAKE_CASE for constants
- Avoid single-letter names except for common cases (i, j for loops)

**Python:**
- snake_case for functions, variables, modules
- PascalCase for classes
- SCREAMING_SNAKE_CASE for constants
- Avoid single-letter names except for common cases

**JavaScript/TypeScript:**
- camelCase for functions, variables
- PascalCase for classes, interfaces, types
- SCREAMING_SNAKE_CASE for constants
- Avoid single-letter names except for common cases

**Go:**
- camelCase for unexported names
- PascalCase for exported names
- Acronyms should be all caps (HTTP, ID, URL)

Report any violations with:
- Identifier name
- Current convention used
- Expected convention
- Line number

If this file doesn't contain identifiers or follows conventions correctly, respond with "PASS".
