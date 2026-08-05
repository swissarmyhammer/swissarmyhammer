---
assignees:
- claude-code
depends_on:
- 01KZ9361Q5W4W38TGRWB01GTZG
- 01KZ935S9GWN207TF50MHCN5HB
position_column: todo
position_ordinal: ff8380
title: missing-docs runners for TS, Swift, Go, and Dart
---
Clone the missing-docs tool-rule pattern to the remaining languages. Each is a rule file in `code-hygiene/rules/` with a `tool` block and `supersedes: missing-docs`.

- TypeScript/JavaScript: eslint with `jsdoc/require-jsdoc`. Generate a flat config in a temp path; pass with `--config`. JSON via `--format json`.
- Swift: swiftlint rule `missing_docs` (opt-in). Generate a config in a temp path. JSON via `--reporter json`.
- Go: revive rule `exported`. Generate the toml config in a temp path.
- Dart: `public_member_api_docs`. `dart analyze` takes no per-run rule flags — this is the generated-config test case. Solve it last.

Each tool rule ships fail/pass fixtures and pinned install commands. Follow the pattern proven by the Rust and Python tool rules (^b01gtzg).

#tool-validators