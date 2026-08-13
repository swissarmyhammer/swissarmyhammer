---
assignees:
- claude-code
position_column: todo
position_ordinal: ffbb80
title: function-length-go sets statements 10000, which removes the data and builder carve-out
---
`builtin/validators/code-hygiene/rules/function-length-go.md` runs `funlen` through golangci-lint at `lines: 250`, `statements: 10000`, `ignore-comments: true`, and declares `supersedes: [function-length]`.

`function-length.md` exempts "Functions that are mostly configuration/data (e.g., builder patterns with many options)" and "Initialization functions that set many fields". `funlen` has two dimensions, lines and statements. A 400-line composite literal is one statement, so the statement dimension WOULD carve it out. The rule sets `statements: 10000` on purpose to turn that dimension off, so the line gate is the only gate and a data-heavy function reports. The exemption is made unreachable by configuration.

Compare `function-length-python`, which selects `PLR0915` — a statement count — and gets the same carve-out for free. The two rules make opposite choices for one prompt rule.

The test carve-out is also dropped: golangci-lint analyses `_test.go` files by default, the temp config sets no `linters.exclusions.rules`, and `funlen` has no test option. A 300-line table-driven `TestFoo` reports.

The generated-code carve-out IS reproduced, from the `linters.exclusions.generated: lax` default.

`//nolint:funlen // <reason>` works. Decide whether the statement dimension comes back, and how tests are exempted.

Found by the `supersedes` survey on ^h7garpc. #tool-validators #objectivity