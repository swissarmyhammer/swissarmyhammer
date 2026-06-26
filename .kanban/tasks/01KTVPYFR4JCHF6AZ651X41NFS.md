---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kw2ynmmb296wx78ggzy0hsbd
  text: |-
    Implemented TOML-aware MCP config writes (TDD: 5 failing tests first → RED confirmed → GREEN).

    Design: added extension-dispatched helpers in `settings.rs`:
    - `is_toml_config(path)` — single `.toml` predicate, shared with status reader.
    - `toml_str_to_json(content)` — TOML→serde_json::Value (extracted; status.rs `read_config_doc` now reuses it instead of its inline toml branch).
    - `json_to_toml_string(value)` (private) — serde_json::Value→TOML via `toml::Value::try_from` + `toml::to_string` (toml 1.1, toml_edit-backed, emits scalars before tables so unrelated keys like `model` are preserved).
    - `read_mcp_config` / `write_mcp_config` — dispatch on extension; non-`.toml` paths delegate to existing `read_json`/`write_json` byte-identically.

    Routed all four writers through the new helpers: `mcp_config::register_mcp_server`, `mcp_config::unregister_mcp_server`, `strategy::generic_register_mcp` (generic_unregister_mcp already routes via unregister_mcp_server). The shared `set_mcp_server_entry`/`remove_mcp_server_entry` are unchanged.

    Tests added: TOML write shape, unrelated-key preservation (register + unregister), round-trip with status `mcp_server_installed`, and a strategy-level Codex `.codex/config.toml` agent test asserting TOML output + idempotent re-register (Ok(false)).

    Verification (all green): `cargo nextest run -p mirdan` = 408 passed, 0 failed; `cargo fmt`; `cargo clippy -p mirdan --all-targets -- -D warnings` clean. Adversarial double-check: PASS (verified ordering proof, blast radius, edge cases).

    Left in `doing` for review.
  timestamp: 2026-06-26T21:51:43.243510+00:00
- actor: claude-code
  id: 01kw306meg5ea8gv8r1135ybbb
  text: |-
    Resolved all 4 review findings on the TOML-aware MCP config writes.

    1. (warning) De-duplicated the TOML/JSON read dispatch. Extracted `settings::dispatch_read_config(path, content) -> Result<Value, RegistryError>` as the single source of truth for the `is_toml_config` branch (TOML via `toml_str_to_json`, else JSONC via `crate::parse_jsonc`). Both `status::read_config_doc` (layers `.ok()` → Option) and `settings::read_mcp_config` (layers file IO + empty/NotFound → empty object) now call it. JSON-path error message ("Invalid JSON in {path}: {e}") and missing/empty behavior kept byte-identical for non-.toml paths.

    2. (nit) Inlined `json_to_toml_string` (single call site) into `write_mcp_config` and deleted the helper. Zero remaining references.

    3. (nit) Trailing newline: investigation showed `toml::to_string` ALREADY emits a trailing newline, so the literal suggested fix `format!("{}\n", toml)` would DOUBLE it (inconsistent with the JSON path's single newline — contradicting the finding's own intent). Implemented the robust version instead: `if !toml.ends_with('\n') { toml.push('\n'); }` — guarantees exactly one trailing newline and also covers the empty-doc edge (""→"\n"). Added tests: `write_mcp_config_toml_ends_with_single_trailing_newline` (ends_with \n, not \n\n), `write_mcp_config_toml_empty_object_gets_trailing_newline` (== "\n"), plus trailing-newline + no-double assertions on the canonical mcp_config.rs TOML-write test.

    4. (nit) Inlined the `codex_agent` test helper (single call site) into `generic_register_mcp_writes_toml_for_toml_config_path` and deleted it; `root` → `dir.path()`, all AgentDef fields identical.

    Verification (all green): `cargo nextest run -p mirdan` = 410 passed, 0 failed (was 408; +2 new tests); `cargo fmt`; `cargo clippy -p mirdan --all-targets -- -D warnings` clean. Adversarial double-check: PASS (verified dispatch is single-source, byte-identical JSON behavior, no double newline, no dead refs).

    Files: crates/mirdan/src/settings.rs, crates/mirdan/src/status.rs, crates/mirdan/src/mcp_config.rs, crates/mirdan/src/strategy/mod.rs. Left in `doing` for review.
  timestamp: 2026-06-26T22:18:28.688250+00:00
position_column: doing
position_ordinal: '8180'
project: mirdan-install
title: 'mirdan: TOML-aware MCP config writes (Codex .codex/config.toml)'
---
## What
All mirdan MCP config writers are JSON-only, but Codex's MCP config is TOML (`.codex/config.toml`, servers key `mcp_servers` — `crates/mirdan/src/agents_default.yaml:268-281`). Today registering an MCP server for Codex would write pretty-printed JSON into a `.toml` file. The status *reader* already dispatches on extension (`crates/mirdan/src/status.rs:630-644` parses `.toml` via `toml::from_str` and converts to `serde_json::Value`); the writers do not.

Affected writers (all funnel through `settings::read_json` / `settings::write_json` in `crates/mirdan/src/settings.rs:36-58`, which call `swissarmyhammer-common`'s JSONC `read_json_file`):
- `mcp_config::register_mcp_server` / `mcp_config::unregister_mcp_server` (`crates/mirdan/src/mcp_config.rs:169` / `:184`) — used by the root-explicit path `install.rs::register_mcp_server_at`.
- `generic_register_mcp` / `generic_unregister_mcp` (`crates/mirdan/src/strategy/mod.rs:378` / `:407`) — the `GenericMcpJsonStrategy` path (its doc comment already claims "JSON/TOML" but the impl is JSON-only).

Approach: add extension-dispatched config read/write helpers (e.g. `read_mcp_config` / `write_mcp_config` in `crates/mirdan/src/settings.rs` or `mcp_config.rs`): for `.toml` paths, parse TOML → convert to `serde_json::Value` (reuse/extract the existing toml→json conversion already used by `status.rs`), mutate via the existing `set_mcp_server_entry` / `remove_mcp_server_entry` (`mcp_config.rs:101-157`, unchanged), then convert back and serialize with the `toml` crate. All four writer functions above switch to the new helpers; JSON behavior is byte-identical to today for non-`.toml` paths.

- [ ] Extension-dispatched read/write helpers (TOML ⇄ serde_json::Value), reusing the status.rs toml→json conversion
- [ ] Route `mcp_config::register_mcp_server` / `unregister_mcp_server` through them
- [ ] Route `generic_register_mcp` / `generic_unregister_mcp` through them
- [ ] Unit tests (see Tests)

## Acceptance Criteria
- [ ] Registering `McpServerEntry { command, args: ["serve"], env: {} }` into a temp `<root>/.codex/config.toml` produces valid TOML containing `[mcp_servers.kanban]` with `command` and `args`, not JSON text
- [ ] Unrelated pre-existing TOML keys in the file (e.g. `model = "..."`) are preserved across register/unregister
- [ ] The write round-trips with the existing TOML status reader: `status.rs` MCP-installed detection reports the server as installed after a write
- [ ] Re-registering an identical entry is a no-op (idempotent, mirrors the existing JSON `Ok(false)` behavior)
- [ ] Existing JSON-path tests in `mcp_config.rs` and `strategy/mod.rs` still pass unchanged

## Tests
- [ ] Unit tests in `crates/mirdan/src/mcp_config.rs` `#[cfg(test)]`: register into temp `config.toml` → parse with `toml::from_str`, assert entry shape; unregister removes it; unrelated keys preserved; idempotent re-register returns no-change
- [ ] Unit test in `crates/mirdan/src/strategy/mod.rs` tests: `GenericMcpJsonStrategy::register_mcp` against a synthetic agent whose `mcp_config.project_path` ends in `.toml` writes TOML
- [ ] Round-trip test: write via the new path, assert the `status.rs` TOML reader sees the server installed
- [ ] `cargo test -p mirdan` passes with 0 failures

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.