---
assignees:
- claude-code
position_column: todo
position_ordinal: ffd580
title: complexity-swift drops the closure, which function-length measures
---
`function-length` states "All Function Types: Methods, closures, lambdas, standalone functions". `complexity-swift` supersedes that prompt rule, and its child configuration names `cyclomatic_complexity` and `function_body_length` alone.

Measured with swiftlint 0.65.0 over one closure of 300 body lines held in a `let`: the run reports nothing. The same 300 lines in a `func` report `Function body should span 250 lines or less`.

swiftlint holds `closure_body_length` for a closure. It is an opt-in rule, and its default gate is `warning: 20` and `error: 100`, which is not the 250 of the `function-length` prompt gate.

Measure `closure_body_length` at 250 over a body of real Swift — Alamofire, swift-nio and vapor are the corpus the `ignores_case_statements` measurement of `complexity-swift` used. Then decide from the measurement whether the child configuration names the rule. A trailing closure of a SwiftUI `body` or of a test builder is the shape to read first: a rule that reports every long trailing closure makes a suppression mandatory on code the prompt rule calls correct, which is the trade `complexity-swift` refuses elsewhere.

The rule body states the gap today, under "What each gate reaches, and what neither reaches". Correct that section with whatever the measurement decides.

Found while measuring the carve-outs on ^h2ezbs7. #tool-validators #objectivity