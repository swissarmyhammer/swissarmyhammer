---
assignees:
- claude-code
position_column: todo
position_ordinal: ffb980
title: dead-code-swift reports test-only helpers, which dead-code exempts by name
---
`builtin/validators/code-hygiene/rules/dead-code-swift.md` runs `swift build --build-tests` then `periphery scan --retain-public ...` and declares `supersedes: [dead-code]`.

`dead-code.md` exempts "**Tests**: test functions and test-only helpers ... and items gated by `#[cfg(test)]` / `mod tests`."

`--build-tests` makes the test harness a caller, which covers the test functions. It also brings the test-support code itself under the gate, and the rule's own measurement says where the findings sit: "**52 of the 74 findings** sit [there] — a long file of `AFError` convenience properties no test ever calls." Those 52 are the "test-only helpers" the prompt rule carves out. periphery has no flag that retains test-support declarations while still counting tests as callers.

The entry-point carve-out is partial. `--retain-objc-accessible`, `--retain-swift-ui-previews` and `--retain-codable-properties` cover the ObjC runtime, `#Preview` and reflection-driven encoding. They do not cover a declaration reached by string-keyed registration or a non-`@objc` plugin entry.

`// periphery:ignore` works but takes no trailing text, so a reason needs a second comment line. 52 suppressions is not a carve-out.

Decide how the rule separates test-support code from product code, or state on the rule why it cannot.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity