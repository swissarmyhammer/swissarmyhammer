---
assignees:
- claude-code
position_column: todo
position_ordinal: a080
project: diagnostics
title: 'Invert lsp↔code-context dependency: relocate LSP client + server specs into swissarmyhammer-lsp'
---
## What
The foundational dependency inversion behind the "one LSP system" invariant. **Today the edge points the wrong way:** `swissarmyhammer-lsp` depends on `swissarmyhammer-code-context` (verified: `crates/swissarmyhammer-lsp/Cargo.toml` has `swissarmyhammer-code-context`; `daemon.rs` does `use swissarmyhammer_code_context::{LspJsonRpcClient, SharedLspClient}`; `types.rs` comment "canonical OwnedLspServerSpec lives in code-context"). The JSON-RPC client (`LspJsonRpcClient` in `crates/swissarmyhammer-code-context/src/lsp_communication.rs`), the `SharedLspClient = Arc<Mutex<Option<LspJsonRpcClient>>>` alias (`lsp_worker.rs`), the server-spec types (`OwnedLspServerSpec`/`LspServerSpec`), the registry (`LSP_REGISTRY`, `load_lsp_servers`, `builtin_lsp_yaml_sources`), and the `builtin/lsp/*.yaml` specs all live in code-context; `swissarmyhammer-lsp` only re-exports them and wraps them in `LspDaemon`/`LspSupervisorManager`.

This task **moves the client + specs + registry DOWN into `swissarmyhammer-lsp`** and **flips the Cargo edge** so `swissarmyhammer-code-context` depends on `swissarmyhammer-lsp` (not vice versa). It is deliberately a single task because a half-done inversion does not compile — the seam IS the compile boundary. It is primarily *relocation*, not new logic.

- Move `lsp_communication.rs` (`LspJsonRpcClient`), the `SharedLspClient` alias, and the server-spec/registry/yaml-loader modules from `swissarmyhammer-code-context` into `swissarmyhammer-lsp`.
- **Extract a transport trait** (e.g. `LspTransport` with `send_request`/`send_notification`/`read_message`) that `LspJsonRpcClient` implements over real child-process stdio. Today `LspJsonRpcClient` (lsp_communication.rs:161) is a concrete struct over `ChildStdin`/`BufReader<ChildStdout>` with NO mock seam — extracting the trait here is what lets the session tasks (g167jrk/3z6g7da) unit-test the open-doc state machine + diagnostics fan-out against an in-memory fake transport, model-free and without a real rust-analyzer.
- Update `swissarmyhammer-lsp` internals (`daemon.rs`, `supervisor.rs`, `registry.rs`, `yaml_loader.rs`, `types.rs`) to use the now-local types; delete the re-export shims that pointed at code-context.
- In `swissarmyhammer-code-context/Cargo.toml` add `swissarmyhammer-lsp`; in `swissarmyhammer-lsp/Cargo.toml` remove `swissarmyhammer-code-context`. Confirm no dependency cycle remains (`cargo tree` shows a single direction lsp ← code-context).
- Keep code-context's public API stable by re-exporting the moved types from `swissarmyhammer-lsp` (`pub use swissarmyhammer_lsp::{LspJsonRpcClient, SharedLspClient, OwnedLspServerSpec, ...}`) so downstream (`swissarmyhammer-tools` code_context tool) still compiles unchanged.
- Update `ARCHITECTURE.md` crate-tier notes to record swissarmyhammer-lsp as the LSP-client owner and the new dependency direction (this captures the layout the missing "file-edit-tools doc" was meant to hold).

## Acceptance Criteria
- [ ] `LspJsonRpcClient`, `SharedLspClient`, `OwnedLspServerSpec`, `LspServerSpec`, and the LSP registry/yaml loader are defined in `swissarmyhammer-lsp`.
- [ ] An `LspTransport` trait exists in `swissarmyhammer-lsp`, implemented by the real stdio client, with an in-memory fake usable by tests in the session tasks.
- [ ] `swissarmyhammer-code-context` depends on `swissarmyhammer-lsp`; `swissarmyhammer-lsp` no longer depends on `swissarmyhammer-code-context`; `cargo tree -i swissarmyhammer-lsp` shows no cycle.
- [ ] code-context still re-exports the moved types so its consumers compile without source changes.
- [ ] `cargo build --workspace` and `cargo clippy --workspace` are clean.
- [ ] ARCHITECTURE.md reflects the inverted edge and the LSP-client home.

## Tests
- [ ] `cargo test -p swissarmyhammer-lsp` — existing daemon/registry/yaml tests pass from their new home; a new unit test exercises the fake `LspTransport` (model-free, <1s).
- [ ] `cargo test -p swissarmyhammer-code-context` — existing LSP-layer + ops tests still pass (esp. `get_diagnostics` unit tests in `ops/get_diagnostics.rs`).
- [ ] `cargo build --workspace` succeeds (regression guard for the dependency flip).

## Workflow
- Use `/tdd` where logic changes; this task is mostly relocation, so leaning on the existing moved tests as the green bar is acceptable. Do the move in one commit that compiles. #diagnostics