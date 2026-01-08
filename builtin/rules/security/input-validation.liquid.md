---
title: Input Validation
description: Check for SQL injection, XSS, command injection, and other input validation vulnerabilities
category: security
severity: error
tags: ["security", "injection", "validation"]
---

Check {{ language }} code for input validation vulnerabilities.

Look for:
- SQL queries constructed with string concatenation or interpolation
- Shell commands built from user input
- File paths constructed from user input without sanitization
- HTML output that includes unescaped user data
- XML parsing without XXE protection
- Deserialization of untrusted data

Common vulnerability patterns:
- Direct string interpolation into SQL: `SELECT * FROM users WHERE id = '${id}'`
- Unvalidated command execution: `exec("ls " + userInput)`
- Path traversal: `readFile("./uploads/" + filename)`
- XSS in HTML: `<div>${userContent}</div>` without escaping

Do not flag:
- Properly parameterized queries using prepared statements
- Sanitized inputs with validation libraries
- Escaped output using framework-provided functions
- Static strings without user input

{% include "_partials/report-format" %}

Report vulnerabilities with:
- Vulnerability type (SQL injection, XSS, etc.)
- Location and code snippet
- Suggested fix using safe APIs

{% include "_partials/pass-response" %}
