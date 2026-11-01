---
assignees:
- claude-code
position_column: todo
position_ordinal: ffff9180
title: test-partitioning rule asks for an integration target the validators crate does not have
---
The review rule test-integrity/test-partitioning reported this finding on the missing-docs-swift card (swift.rs:227): "Integration test inside the unit target: runs a project .swift-format through the real tool via swift_gate_reporting_rules. Move this test to the separate integration target that runs the real Swift toolchain."

The validators crate has no separate integration target. It has no tests/ directory, no feature gate and no integration module. Every real-tool test for Swift, Rust, Go, Dart, Python and TypeScript sits in the unit target under crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped, and they share private helpers from shipped.rs. The rule measures a target that does not exist here.

Decision from the user: the finding was released on the missing-docs-swift card. The rule is wrong for this repo, so fix the rule. Do not build a new test harness.

To do:
- Find the test-partitioning rule (a prompt rule under builtin/validators/).
- Make it match the repo convention: real-tool tests in the shipped unit target are correct.
- Check whether the rule can be decided by a tool or a structural fact (annotation, config) and not by judgment. The preferred fix removes LLM judgment.
- Add a test through the real review path that proves the rule stays silent on a real-tool test in the shipped unit target. #tool-validators #validator-quality