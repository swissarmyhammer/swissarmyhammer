---
assignees:
- claude-code
position_column: todo
position_ordinal: df80
title: Lowercase the capitalized error Display messages in the shared library crates
---
`builtin/validators/rust/rules/error-handling.md` states: Display messages on errors are lowercase, with no trailing punctuation.

`^p4mp9n6` swept swissarmyhammer-tools, mirdan and agent-client-protocol-extras. That leaves the shared library crates below them still capitalized, so the same failure now reads two different ways depending on which layer produced it.

Concrete drift the sweep created:

- `crates/mirdan/src/mcp_config.rs` now says `invalid JSON in {}: {}`, but `crates/swissarmyhammer-common/src/json.rs` still says `Invalid JSON in {}: {}` for the same class of failure. `crates/mirdan/src/settings.rs` carries a doc comment quoting the old capitalized text.
- `crates/swissarmyhammer-tools/.../files/glob/mod.rs` now says `invalid glob pattern: {}`, but `crates/swissarmyhammer-common/src/glob_utils.rs` still says `Invalid glob pattern`, with two tests pinning that casing.

## Scope

Sweep these crates for error Display messages that start with a capital:

- swissarmyhammer-common (`src/error.rs`, `src/mcp_errors.rs`, `src/glob_utils.rs`, `src/json.rs`, `src/fs_utils.rs`, `src/frontmatter.rs`)
- swissarmyhammer-store, swissarmyhammer-views, swissarmyhammer-kanban, swissarmyhammer-config, swissarmyhammer-templating, swissarmyhammer-perspectives, swissarmyhammer-validators, swissarmyhammer-git, swissarmyhammer-code-context, swissarmyhammer-shell, swissarmyhammer-web, swissarmyhammer-treesitter, swissarmyhammer-agents, swissarmyhammer-skills, llama-common, model-loader, acp-conformance

Apply the same rule `^p4mp9n6` used: lowercase the first character unless the first word is an all-caps acronym (`I/O`, `JSON`, `ZIP`), a CamelCase identifier, or a proper noun (`Git`). Strip a trailing full stop. Leave UI titles, log lines and `.expect()` panic text alone.

Update `crates/mirdan/src/settings.rs`'s doc comment once `swissarmyhammer-common` changes.

## Acceptance

- No `#[error("[A-Z]` or `write!(f, "[A-Z]` Display message in these crates starts with a plain capital word.
- Every test pinning the old casing is updated in the same change.
- `cargo nextest run` green for each package touched; `cargo clippy --all-targets -- -D warnings` clean for each. #bug