---
assignees:
- claude-code
position_column: todo
position_ordinal: ff9a80
title: decide whether a manifest-scoped validator set should host unused-dependency tools
---
`cargo machete` was rejected from the `code-hygiene` set (^teemmch). One of the two reasons was structural, and it applies to every unused-dependency tool, in every language:

- Machete reports findings whose path is a `Cargo.toml`.
- `code-hygiene` matches `@file_groups/source_code`, and `builtin/file_groups/source_code.yaml` names source extensions only. It has no manifest pattern.
- `Rule::matches` is narrow-only: a rule never matches a file its set does not match. A `workspace`-scope run is filtered the same way, because `execute_tool_runs` keeps only findings whose path is a matched changed file.

So no manifest finding can ever reach a report from a source-code set.

Decide one of these:

1. Add a new validator set that matches manifests (`Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`). It would host unused-dependency rules and any other manifest check.
2. Leave unused dependencies to CI, outside the review engine.

Do NOT widen `@file_groups/source_code` to include manifests. That group is the match of every source validator set — code-security, completeness, rust, naming, test-integrity, reuse — and widening it would put manifests in scope for all of them at once.

If option 1 is chosen, note that `cargo machete` still needs an accuracy decision. Its `--with-metadata` mode reports `tauri-build` unused for `kanban-app` and `mirdan-app`, and both `build.rs` files call `tauri_build::build()`. Its default mode misses that dependency kind because it scans source text for the crate name, so it cannot see a renamed or feature-gated use either.

#tool-validators