---
assignees:
- claude-code
position_column: todo
position_ordinal: ffa680
title: 'manifests validator set: host cargo machete (unused dependencies)'
---
The dead-code evaluation (^teemmch) rejected `cargo machete` from `code-hygiene` for a scoping reason, not a quality reason: every machete finding names a `Cargo.toml`, and code-hygiene matches source code. The VALIDATOR.md record states the path: "A validator set that matches manifests could host this tool."

Build that set:
- New builtin set `manifests/` whose `match.files` covers manifests (`**/Cargo.toml`; leave room for `**/package.json` later).
- Tool rule `unused-dependencies-rust`: `cargo machete` at workspace scope, findings mapped to the `Cargo.toml` that declares the unused dependency. No `supersedes` — no prompt rule covers this today.
- Inline suppression: machete honors `[package.metadata.cargo-machete] ignored = [...]` in the crate manifest. State it in the rule body.
- Pin the machete version in `install.commands`.
- Fixture pair: a fail fixture manifest with a dependency no fixture source uses; a pass fixture whose one dependency is used. Follow the cargo fixture-package shape the code-hygiene fixtures already use.
- The set only fires when a manifest changed — that is the correct trigger for a dependency question.

#tool-validators