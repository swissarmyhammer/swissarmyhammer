---
assignees:
- claude-code
position_column: todo
position_ordinal: '8280'
project: plugin-arch
title: 'plugin: scaffold the swissarmyhammer-plugin crate'
---
## What
Create the new `swissarmyhammer-plugin` workspace crate — the plugin platform itself. This task is the empty shell + workspace wiring only; subsequent tasks fill in the modules.

- `crates/swissarmyhammer-plugin/Cargo.toml` — name, workspace inheritance for version/edition/etc. Dependencies it will need (add now, used later): `deno_core`, `deno_ast`, `rmcp`, `tokio`, `async-trait`, `serde`, `serde_json`, `thiserror`, `tracing`, `swissarmyhammer-directory`. Per the workspace rule in root `Cargo.toml`: NO feature flags.
- Add `"crates/swissarmyhammer-plugin"` to root `Cargo.toml` `[workspace] members`.
- `crates/swissarmyhammer-plugin/src/lib.rs` — crate-level doc comment summarizing the platform (register/unregister + generic dispatch), and `mod` declarations stubbed for the modules that follow: `registry`, `dispatcher`, `server` (McpServer trait + transports), `runtime`, `host`, `ledger`, `codegen`. Stub modules can be empty files for now.
- `crates/swissarmyhammer-plugin/src/error.rs` — the platform `Error` enum (`thiserror`) with the variants the doc names: `UnknownServer`, `UnknownTool`, `UnknownOperation`, `ServerNameTaken(String)`, `ServerUnavailable`, `PluginReloaded`. `pub type Result<T> = std::result::Result<T, Error>;`
- `tests/` directory created with an `integration/` subdir placeholder.

## Acceptance Criteria
- [ ] `swissarmyhammer-plugin` is a workspace member; `cargo build -p swissarmyhammer-plugin` succeeds.
- [ ] `Error` enum and `Result` alias exist and are exported from `lib.rs`.
- [ ] Stub module files exist and are declared; crate has no feature flags.

## Tests
- [ ] `cargo build -p swissarmyhammer-plugin` and `cargo build --workspace` succeed.
- [ ] A trivial `#[test]` in `error.rs` asserting `Error::ServerNameTaken("x".into())` Displays a non-empty message — proves the crate compiles and links.

## Workflow
- Use `/tdd` for the error-Display test; the scaffold itself is verified by a clean workspace build.