---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzfgwj3safkqmm6khkbj9v76
  text: |
    ### decision — option 1, manifest-scoped set

    The user decided: build it. A cargo tool needs no placement debate. Install it if it is missing, the same as `cargo nextest`.

    Note for the implementer: installation was never the blocker. The blocker is file matching. `code-hygiene` matches `@file_groups/source_code`, that group names source extensions only, and `Rule::matches` is narrow-only, so a finding whose path is `Cargo.toml` never reaches a report. Auto-install alone does not fix this.

    Do this:

    - Add a manifest file group: `Cargo.toml`, `package.json`, `pyproject.toml`, `go.mod`.
    - Add a validator set that matches that group and hosts unused-dependency rules.
    - Leave `builtin/file_groups/source_code.yaml` unchanged. The card forbids widening it, because every source validator set matches it.
    - Make the tool doctorable and auto-installable, the same as the other tool rules.

    Open accuracy point, to settle during implementation, not by a new question: `cargo machete --with-metadata` calls `tauri-build` unused for `kanban-app` and `mirdan-app`, and both `build.rs` files call `tauri_build::build()`. Default mode scans source text for the crate name, so it misses renamed, feature-gated, and build-script uses. Pick the mode that gives no false positive on this repo and pin the fixture pair to prove it.
  timestamp: 2026-08-08T01:48:03.065905+00:00
- actor: claude-code
  id: 01kzfgwytzcery6a8fd93d5p63
  text: |
    ### decision — rejected, closing the card

    This comment replaces the comment above it. That earlier comment recorded the wrong decision.

    The user decided: no manifest-scoped validator set, and no decision exercise. Unused-dependency tooling stays out of the review engine.

    - No new file group.
    - No new validator set.
    - `builtin/file_groups/source_code.yaml` stays unchanged.
    - `cargo machete` stays rejected, as ^teemmch already recorded.

    No code change. The card is archived.
  timestamp: 2026-08-08T01:48:16.095450+00:00
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