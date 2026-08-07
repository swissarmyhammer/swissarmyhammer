---
assignees:
- claude-code
depends_on:
- 01KZEBACVE127AV1BTD3DFHNXG
position_column: todo
position_ordinal: ff9180
title: 'complexity tool rules: TypeScript + Swift + Go'
---
Extend the complexity and function-length tool rules to the other languages. Follow the pattern from ^3dfhnxg.

TypeScript/JavaScript — eslint, files scope:
- eslint-plugin-sonarjs `cognitive-complexity` at 15 (the same Sonar metric the tree-sitter probe computes) plus core `max-lines-per-function` at 250 with skipBlankLines and skipComments.
- The run script writes a temporary flat config and passes `--config <tmp> --no-config-lookup`. Pin the eslint and plugin versions in `install.commands`.
- One run, `supersedes: [cognitive-complexity, function-length]`.

Swift — swiftlint, files scope:
- `cyclomatic_complexity` and `function_body_length` in one run.
- The run script writes a temporary `.swiftlint.yml` and passes `--config`. Use `--reporter json` piped through jq.
- `supersedes: [cognitive-complexity, function-length]`.

Go — files scope:
- `gocognit -over 15` for complexity (Sonar cognitive metric). Supersedes `cognitive-complexity`.
- Pick a function-length tool during the work. If no standalone tool is healthy, Go keeps the `function-length` prompt rule. Record the decision in the rule body.

Dart — no tool rule. DCM (dart_code_metrics) is commercial. Dart keeps the probe + prompt path. State this in the code-hygiene VALIDATOR.md so a reviewer does not file it as a gap.

Every new rule ships a fail/pass fixture pair and shows doctor rows.

#tool-validators