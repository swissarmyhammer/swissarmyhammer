---
name: no-log-truncation
description: Detect truncation of log messages which loses diagnostic information
trigger: Stop
severity: error
tags:
  - code-quality
  - logging
---

# No Log Truncation

Check the changed files for log message truncation patterns that lose diagnostic information.

## What to Look For

1. **Explicit truncation for "readability"**:
   - Comments mentioning truncation "for readability" or "for log readability"
   - Taking substrings or slices of log messages before logging
   - Using `.take(n)` or `[..n]` on message strings

2. **Length-based truncation**:
   - Checking message length and truncating if over a threshold
   - Adding "..." or ellipsis to indicate truncation
   - Using substring operations on error/log messages

3. **Format truncation**:
   - Using format specifiers that limit string width (e.g., `{:.100}`)
   - Limiting output in log macros

## Why This Matters

- Truncated logs make debugging harder - you lose the context
- The "readability" gained is false economy - grep and log tools handle long lines
- Critical diagnostic information often appears at the end of messages
- Modern log systems handle arbitrary-length messages well
