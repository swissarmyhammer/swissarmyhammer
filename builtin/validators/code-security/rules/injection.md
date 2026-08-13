---
name: injection
description: Detect SQL injection, XSS, command injection, and other input validation vulnerabilities
---

# Injection Rule

You are a security validator that checks code for input validation vulnerabilities including SQL injection, XSS, command injection, and other injection attacks. A confirmed injection sink is a **blocker**.

## What to Check

Examine the file content for these vulnerability patterns:

### 1. SQL Injection
- SQL queries constructed with string concatenation or interpolation
- Template literals or f-strings containing SQL with user input
- Example: `SELECT * FROM users WHERE id = '${id}'` or `f"SELECT * FROM users WHERE id = '{user_id}'"`

### 2. Command Injection
- Shell commands built from user input without sanitization
- `exec()`, `system()`, `popen()`, `subprocess` with unsanitized strings
- Example: `exec("ls " + userInput)` or `os.system(f"rm {filename}")`

### 3. Path Traversal
- File paths constructed from user input without sanitization
- No validation for `..` or absolute paths
- Example: `readFile("./uploads/" + filename)` or `open(f"data/{user_path}")`

### 4. Cross-Site Scripting (XSS)
- HTML output that includes unescaped user data
- Template rendering without auto-escaping
- Example: `<div>${userContent}</div>` or `innerHTML = userInput`

### 5. XML External Entity (XXE)
- XML parsing without disabling external entities
- Example: `etree.parse(user_xml)` without `resolve_entities=False`

### 6. Deserialization
- Deserialization of untrusted data using unsafe methods
- Example: `pickle.loads(user_data)`, `yaml.load(user_input)` without safe loader

## Before you report

A matched pattern is a candidate, not a finding. One gate stands between them.
Pass it, or stay silent.

**Quote the sink from the file.** Open the line you are about to name. Copy the
interpolation as it stands on disk — every quote character, every escape
character, in order — into the finding. A finding you cannot quote is a finding
you did not verify.

**Then read the quoted text for the treatment you are about to ask for.** The
treatment lives inside the string literal, so a safe sink and an unsafe one have
the same shape. Read the characters, not the shape:

- `format!("exec {real} \"$@\"")` interpolates the path bare. Report it.
- `format!("exec \"{real}\" \"$@\"")` interpolates the path inside double
  quotes. The treatment is present. Stay silent.

The same reading decides every other sink. A `?` or `$1` placeholder is not
concatenation. `html.escape(value)` inside the template is the escape this rule
asks for. `safe_load` is not `load`.

Report only what the quoted text lacks. A finding that asks for a treatment the
line already holds cannot be satisfied by any edit.

Measured on 2026-08-12, on one review. A finding named
`verify_shipped_tree_breaks_without` in
`crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs` for
command injection, and asked that `exec {real} "$@"` become `exec "{real}"
"$@"`. That line already read `exec \"{real}\" \"$@\"` — commit `bf0cd8d34`
quoted it earlier the same day. The rule reported the fix as the defect.

## Exceptions (Don't Flag)

- **Treatment already present**: The quoted line carries the escaping, quoting,
  or placeholder this rule asks for. Read the line to answer this; the shape of
  the sink cannot.
- **Parameterized queries**: Properly using prepared statements with placeholders (`?`, `$1`, `:name`)
- **Sanitized inputs**: Using validation libraries like `validator.js`, `bleach`, `html.escape()`
- **Escaped output**: Using framework-provided escaping functions
- **Static strings**: Hardcoded strings without user input concatenation
- **Safe APIs**: Using `subprocess.run(..., shell=False)` with list arguments

Note: Do not exempt code based on the filename containing `test`, `_test`, `test_`, `.spec.`, or `.test.`. The dispatcher decides whether a file is a test via `@file_groups/test_files`; this rule's job is to flag injection patterns wherever they appear. Test fixtures and helpers can contain real injection vulnerabilities (e.g. unsanitised `format!` SQL builders shared from a `tests/` helper into production), so flag them too.

Include:
- Vulnerability type (SQL injection, XSS, command injection, path traversal, XXE, deserialization)
- Location (line number and function/method name if available)
- Brief description of the vulnerable pattern
- Suggested fix using safe APIs
