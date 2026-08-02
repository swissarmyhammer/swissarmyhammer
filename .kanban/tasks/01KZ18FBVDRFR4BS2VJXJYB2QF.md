---
assignees:
- claude-code
position_column: todo
position_ordinal: e380
title: complexity scorer covers only Rust — map the remaining source_code languages
---
The `complexity` probe computes Sonar cognitive complexity from the tree-sitter parse (see `crates/swissarmyhammer-sem/src/parser/plugins/code/complexity.rs`). It has one `ComplexitySpec` row today: Rust.

Every other language the `complexity` validator matches (`@file_groups/source_code`) reports **not computed**. That is deliberate and safe — a missing mapping never reads as a score of zero — but it means the agent still judges those languages by eye, which is the drift ^k5wsxh0 removed for Rust.

## Work

Add a `ComplexitySpec` row per language. `swissarmyhammer-sem` already carries the grammars and the `LanguageConfig` for: typescript, tsx, javascript, python, go, java, c, cpp, ruby, c_sharp, php, fortran, swift, elixir, bash.

For each row, verify the node kinds against the real grammar (parse a sample and read the s-expression) rather than guessing. The existing Rust row was built that way.

Each language needs the same test set the Rust row has:

- a `match`/`switch` scores once and its arms open no nesting level
- an if/else-if/else chain is flat
- nested loops deepen the score
- a boolean run scores once, a mixed run twice
- the test marker at the definition exempts the function
- repeated scoring never drifts

## Acceptance

- Every extension in `builtin/file_groups/source_code.yaml` that `swissarmyhammer-sem` can parse has a spec row.
- A language with no grammar still reports not-computed, never zero.
- The per-language node kinds are verified against the grammar, not assumed.

#bug #review