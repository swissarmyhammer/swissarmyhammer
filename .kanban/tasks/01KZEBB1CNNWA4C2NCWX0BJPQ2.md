---
assignees:
- claude-code
position_column: todo
position_ordinal: ff9280
title: magic-numbers tool rules + split the data-driven prompt rule
---
Step 1 — split the `data-driven` prompt rule in `builtin/validators/code-hygiene`:
- `data-driven` keeps the table check (a match/if chain over a known set is a table). No tool can make that judgment.
- A new `magic-numbers` prompt rule takes the repeated-literal and repeated-configuration checks. It keeps the same carve-outs: 0, 1, -1, conventional values, and one-off literals in an obvious context.

Step 2 — tool rules that supersede `magic-numbers`:
- Python: ruff PLR2004 with `--isolated`.
- TypeScript/JavaScript: eslint `no-magic-numbers` with ignore list [0, 1, -1] in a temporary config.
- Swift: swiftlint `no_magic_numbers` — an opt-in rule; turn it on in the temporary config.
- Go: `mnd`.
- Rust: no healthy lint exists. Rust keeps the `magic-numbers` prompt rule.
- Dart: the check needs a custom_lint package. Dart keeps the prompt rule.

Compare each tool's default ignore behavior with the prompt carve-outs before you set thresholds. A tool that flags every inline literal makes noise, and noise kills the gate.

Every tool rule ships a fail/pass fixture pair and shows doctor rows.

#tool-validators