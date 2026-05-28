---
assignees:
- claude-code
depends_on:
- 01KSMXKZM1NZV1QH0SSKAP0V4P
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb280
title: 'Doctor/install: accept JSONC anywhere we parse user-written JSON (Postel''s law)'
---
## What

`cargo run --bin sah -- init user` failed for Zed AI with:

```
⚠ failed to install MCP for Zed AI: Validation error: Invalid JSON in
  /Users/wballard/.config/zed/settings.json: expected value at line 1 column 1
```

Zed's `settings.json` is JSONC (JSON with `//` comments and trailing commas), not strict JSON — and Zed ships a default file that opens with a comment block. VS Code's `settings.json` and `.vscode/mcp.json` are also JSONC by convention. Strict `serde_json::from_str` rejects all of them.

Per Postel's law: **be liberal in what we accept**. Anywhere we *parse* user-written JSON config we should accept JSONC. (Writing is a separate concern — we keep writing strict JSON.)

### Call sites that need conversion

Audited via `grep -n 'serde_json::from_str' crates/mirdan/src/` plus the doctor detector path:

- `crates/mirdan/src/settings.rs::load_settings_json` (≈ line 35) — the **direct cause** of the Zed failure; reads any agent's settings.json during install.
- `crates/mirdan/src/mcp_config.rs::load_plugin_json` (≈ line 200) — reads agent plugin metadata.
- `crates/mirdan/src/mcp_config.rs::*` — any other `from_str` call here that reads a user-written file (audit the file end to end; many `from_str`s in this file may be on writes, in-memory blobs, or http response bodies — keep those strict).
- `crates/mirdan/src/status.rs::read_config_doc` — the JSON branch of the TOML-aware reader added in 01KSMXHQ. Same Postel principle: the *detector* reads the same user-written config the *installer* writes to.

Out of scope (do NOT touch):
- Network/HTTP MCP response parsing (claude-agent SSE handlers etc.) — protocol-level JSON, not user config; strict parsing is correct.
- `serde_json::from_str` in tests asserting strict JSON output we produced ourselves.
- `lockfile.rs`, `auth.rs`, `registry/client.rs`, `package_type.rs`, `git_source.rs`, `sync.rs`, `install.rs`, `new.rs` — audit each to confirm whether it reads user-written content; if not, leave strict.

### Design

Introduce a small helper in `crates/mirdan/src/lib.rs` (or a new `crates/mirdan/src/jsonc.rs`):

```rust
/// Parse a string as JSONC (JSON with `//` and `/* */` comments and trailing
/// commas) into a `serde_json::Value`. This is the lenient input format we
/// accept anywhere we read user-written JSON config — agents like Zed and
/// VS Code routinely ship JSONC even when the file extension is `.json`.
/// Writing still uses strict serde_json::to_string.
pub fn parse_jsonc(content: &str) -> Result<serde_json::Value, ParseError> { ... }
```

Implementation: prefer an existing workspace crate if one already provides JSONC. Candidates in priority order:
1. `jsonc-parser` (pure Rust, ~200KB, fastest) — strip-comments-then-serde_json
2. `serde_jsonc` (drop-in but unmaintained; only if 1 isn't preferred)
3. `json5` (supports JSONC superset, slightly slower but well-maintained)

Check `Cargo.lock` first for any of these. If none present, add `jsonc-parser` as a new dep on `crates/mirdan` (or workspace) and gate it behind nothing — JSONC is now the read path.

Then route every read site (the three identified above plus anything the audit surfaces) through `parse_jsonc`. The error type should still surface "file path + parse error" to the user — keep the error context the existing call sites already produce.

### Files

- `crates/mirdan/src/lib.rs` (or `crates/mirdan/src/jsonc.rs` and `pub mod jsonc;` in lib.rs) — the helper
- `crates/mirdan/Cargo.toml` — JSONC dep
- `crates/mirdan/src/settings.rs` — replace `serde_json::from_str` with `parse_jsonc`
- `crates/mirdan/src/mcp_config.rs` — same for the user-config reads (verify each call site)
- `crates/mirdan/src/status.rs::read_config_doc` — JSON branch uses `parse_jsonc`

## Acceptance Criteria

- [ ] `parse_jsonc("// comment\n{\"x\": 1,}")` returns `Ok(Value::Object({"x": 1}))` — comments AND trailing commas accepted.
- [ ] `parse_jsonc("{\"x\": 1}")` returns the same as `serde_json::from_str` — plain JSON is fully backward-compatible.
- [ ] `parse_jsonc("not json")` returns an `Err` whose message resembles the existing `serde_json` shape (line/column) so user-facing error messages don't regress.
- [ ] `sah init user` on a machine with a JSONC-formatted `~/.config/zed/settings.json` installs the MCP entry successfully (regression for the reported failure).
- [ ] `mirdan status` / `sah doctor` correctly resolve `Component::Mcp` for an agent whose config is JSONC.
- [ ] No JSON *writing* changed; we still emit strict JSON via `serde_json::to_string_pretty`.
- [ ] All call sites identified in the audit are routed through `parse_jsonc`; none are missed (use `grep` after the change to confirm no `serde_json::from_str` reads a user-written file).

## Tests

- [ ] `parse_jsonc` unit tests in `crates/mirdan/src/jsonc.rs::tests` (or wherever the helper lives):
  - `test_parse_jsonc_plain_json`
  - `test_parse_jsonc_line_comments`
  - `test_parse_jsonc_block_comments`
  - `test_parse_jsonc_trailing_commas`
  - `test_parse_jsonc_invalid_returns_error`
- [ ] `crates/mirdan/src/settings.rs::tests::test_load_settings_with_comments` — write a tempdir settings.json with `// comment\n{"foo": 1}`, call `load_settings_json`, assert success.
- [ ] `crates/mirdan/src/status.rs::tests::test_mcp_installed_jsonc_json_branch` — write a `.json` file (not `.toml`) with leading `//` comment and a sah server entry, assert `mcp_server_installed` returns `true`.
- [ ] Update the existing Zed-flavored MCP install path test (or add one in `crates/mirdan/src/mcp_config.rs::tests`) that exercises a JSONC settings.json end to end.
- [ ] Gates: `cargo test -p mirdan`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`.

## Workflow

Use `/tdd`. Write `parse_jsonc` unit tests first (they fail — function doesn't exist). Then write the per-call-site regression test for `settings.rs` (it fails on the JSONC input today). Then implement `parse_jsonc`, route the call sites, and watch everything go green.

Reproduce the Zed bug first if possible — write the offending `~/.config/zed/settings.json` shape into a tempdir, run the install path against it, watch it fail with "expected value at line 1 column 1", then fix.

## Depends on

- 01KSMXKZM1NZV1QH0SSKAP0V4P (legacy-check deletion; ensures we're not changing the same files mid-flight) #init-doctor