---
assignees:
- claude-code
position_column: todo
position_ordinal: ffa680
title: 'manifests validator set: host cargo machete (unused dependencies)'
---
## STANDING ORDER

This card is a decision, not a proposal. Your job is to implement it exactly.
- Do NOT re-reject cargo machete. The ^teemmch rejection was a scoping technicality, and it is reversed on that card.
- Do NOT file a follow-up card in place of doing the work.
- Stop and escalate ONLY when a step is impossible. Report the exact command and its output.

## The work

The dead-code evaluation (^teemmch) rejected `cargo machete` from `code-hygiene` for a scoping reason, not a quality reason: every machete finding names a `Cargo.toml`, and code-hygiene matches source code. Build the set that hosts it:

- New builtin set `manifests/` whose `match.files` covers manifests (`**/Cargo.toml`; leave room for `**/package.json` later).
- Tool rule `unused-dependencies-rust`: `cargo machete` at workspace scope, findings mapped to the `Cargo.toml` that declares the unused dependency. No `supersedes` — no prompt rule covers this today.
- Inline suppression: machete honors `[package.metadata.cargo-machete] ignored = [...]` in the crate manifest. State it in the rule body.
- Pin the machete version in `install.commands`.
- Fixture pair: a fail fixture manifest with a dependency no fixture source uses; a pass fixture whose one dependency is used. Follow the cargo fixture-package shape the code-hygiene fixtures already use.
- The set only fires when a manifest changed — that is the correct trigger for a dependency question.

#tool-validators #dead-code #objectivity