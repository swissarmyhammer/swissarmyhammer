---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffcd80
title: 'NIT: no tests cover the new span fields or completion log event'
---
Skipped — tracing-test is not in swissarmyhammer-tools dev-deps. Adding it plus a mock subscriber test for a nit-level logging finding is disproportionate. The logging code is exercised by existing integration tests; it just isn't asserted on at the span-field level.