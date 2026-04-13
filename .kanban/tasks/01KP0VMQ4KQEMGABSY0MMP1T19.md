---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffc480
project: kanban-mcp
title: 'sah-cli: add build.rs, retire generate_docs.rs binary'
---
## What

Create `swissarmyhammer-cli/build.rs` for man pages, shell completions, and doc reference generation at build time, matching shelltool-cli/build.rs and code-context-cli/build.rs. Retire `generate_docs.rs` as a separate binary — the build.rs approach is the consistent pattern.

## Acceptance Criteria
- [x] `swissarmyhammer-cli/build.rs` exists using `build-support/doc_gen.rs`
- [x] `generate_docs.rs` binary removed or deprecated
- [x] `cargo build -p swissarmyhammer-cli` generates docs, man page, completions
- [x] `[build-dependencies]` added: clap, clap-markdown, clap_mangen, clap_complete

## Implementation Notes

The swissarmyhammer-cli `cli.rs` was not self-contained — it referenced `crate::commands::*::DESCRIPTION` constants and `swissarmyhammer::PromptSource` / `swissarmyhammer_common::lifecycle::InitScope`. A `build.rs` that includes `cli.rs` via `#[path = ...]` cannot resolve those library-crate paths, so the following refactor was required to make the pattern match the other CLIs:

- Created `swissarmyhammer-cli/src/commands/agent/description.md` (extracted from the inline `const` in `agent/mod.rs`).
- Replaced each `commands::*::DESCRIPTION` reference in `cli.rs` with a direct `include_str!("commands/*/description.md")` so `cli.rs` depends only on `clap` and `std`.
- Moved `pub use swissarmyhammer::PromptSource`, the `PromptSourceArg <-> PromptSource` From impls, the `InstallTarget -> InitScope` From impl, and the related conversion test out of `cli.rs` into a new `swissarmyhammer-cli/src/cli_conversions.rs` module.
- Registered `cli_conversions` in both `lib.rs` and `main.rs` (the crate compiles the binary modules separately from the library modules, so the impls need to exist in both compilation units).
- Updated `list.rs` to import `PromptSource` from `crate::cli_conversions` instead of the old cli re-export.
- Removed the now-unused `DESCRIPTION` constants from `commands/doctor/mod.rs`, `commands/model/mod.rs`, and `commands/validate/mod.rs` (kept the ones still referenced by `dynamic_cli.rs` — `commands::agent::DESCRIPTION` — and the `#[cfg(test)]` one in `commands/serve/mod.rs`).
- Added the canonical `swissarmyhammer-cli/build.rs` modeled on `shelltool-cli/build.rs`: generates markdown with the `swissarmyhammer/tap/swissarmyhammer` brew formula, man page, and shell completions.
- Removed `[[bin]] sah-generate-docs` from `Cargo.toml`, deleted `src/generate_docs.rs`, moved `clap-markdown` and `clap_mangen` from `[dependencies]` to `[build-dependencies]`, and added `clap` and `clap_complete` to `[build-dependencies]`.
- Updated `build-support/doc_gen.rs` header comment to reflect the new usage (all CLIs now use `build.rs`).

## Verification

- `cargo build -p swissarmyhammer-cli` — clean build, zero warnings
- `cargo build -p swissarmyhammer-cli --all-targets` — clean build
- `cargo clippy -p swissarmyhammer-cli --all-targets` — clean
- `cargo test -p swissarmyhammer-cli --lib` — 447 passed, 0 failed
- `cargo test -p swissarmyhammer-cli --bins` — 426 passed, 0 failed
- `cargo test -p swissarmyhammer-cli --test cli_tests` — 231 passed, 0 failed
- `cargo test -p swissarmyhammer-cli --test cli_integration` — 1 passed, 0 failed
- `cargo test -p swissarmyhammer-cli --test kanban_cli_tests` — 16 passed, 0 failed
- `cargo build --workspace` — clean build
- Verified generated output files exist: `doc/src/reference/sah-cli.md`, `docs/sah.1`, `completions/sah.bash`, `completions/_sah`, `completions/sah.fish`
- `target/debug/sah --version` prints `swissarmyhammer 0.12.11`
- `target/debug/sah-generate-docs` no longer produced
