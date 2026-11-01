---
assignees:
- claude-code
position_column: todo
position_ordinal: ffff8580
title: 'Flaky: the_shipped_dart_missing_docs_tool_rule_reports_every_fail_fixture_line doubles findings in a full run'
---
## Problem
`swissarmyhammer-validators review::tool_rules::tests::shipped::missing_docs::the_shipped_dart_missing_docs_tool_rule_reports_every_fail_fixture_line` fails in a full run of `cargo nextest run -E 'rdeps(swissarmyhammer-lsp) | rdeps(swissarmyhammer-code-context)'` (5226 tests). The assertion at `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped.rs:434` expects 5 findings and gets 10. Each fail-fixture line (for example `class UndocumentedClass {`) has two findings.

The test passes when it runs alone on the same tree (1/1, 31.7 s, 2026-10-17).

## Probable cause
Two tests run the Dart tool on the same fixture directory, or on the same analysis output, at the same time. The findings of one run then go into the other run. Examine the shipped tool-rule test harness for a shared temp directory, a shared `dart analyze` cache, or a shared output file.

## Acceptance criteria
- The test passes in a full parallel run, three runs in sequence.
- Each tool-rule test uses its own directory and its own output. #test-failure