---
title: No Dangerous eval() Usage
description: Detects dangerous eval() or exec() usage that can lead to code injection
category: security
severity: error
tags: ["security", "code-injection", "eval"]
---

Check for dangerous eval() or exec() usage in {{ language }} code.

Flag any use of:
- eval() function with user input or dynamic strings
- exec() function with user input or dynamic strings
- Function() constructor with dynamic code
- setTimeout() or setInterval() with string arguments (JavaScript)
- compile() or exec() with untrusted input (Python)
- Any similar dynamic code execution patterns in {{ language }}

If this file type doesn't support these patterns (e.g., markdown, TOML, JSON), respond with "PASS".

If you find dangerous eval/exec usage:
- Report the line number
- Explain why it's dangerous
- Suggest safer alternatives (e.g., using proper parsers, allowlists, or avoiding dynamic execution)

If no dangerous usage is found, respond with "PASS".

Code to analyze:
```
{{ target_content }}
```
