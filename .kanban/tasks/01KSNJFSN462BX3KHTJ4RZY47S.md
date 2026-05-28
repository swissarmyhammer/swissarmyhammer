---
assignees:
- claude-code
depends_on:
- 01KSNANRDMEDCD5N8VDWAF4JHF
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb380
title: 'Doctor/install: Zed-aware MCP — probe configured servers_key + write source:"custom"'
---
## What

After the JSONC fix (01KSNANR) `sah init user` no longer fails on Zed's settings.json — but `sah doctor` still reports `Zed AI · {project,user} · MCP server: missing`. Two real bugs make Zed MCP both undetectable and unloadable:

### Bug 1: detector hardcodes the server-key list

`crates/mirdan/src/status.rs::mcp_server_installed` probes only `mcpServers`, `servers`, `mcp_servers`. It never consults `AgentDef.mcp_config.servers_key` — the doc comment even acknowledges this gap. For Zed (whose `servers_key` is `context_servers`) the detector returns `false` even when the entry is correctly installed.

### Bug 2: writer omits Zed's required `source: "custom"`

Per Zed's stable schema (PR zed-industries/zed#33539, current docs at https://zed.dev/docs/ai/mcp), a custom stdio MCP entry under `context_servers` requires `"source": "custom"`:

```jsonc
"context_servers": {
  "sah": {
    "source": "custom",
    "command": "sah",
    "args": ["serve"],
    "env": {}
  }
}
```

When `source` is omitted Zed treats the entry as `extension` (assumes a Zed-marketplace extension provides the server) and silently ignores it. Our `McpServerEntry` struct in `crates/mirdan/src/mcp_config.rs` has no `source` field, so every Zed install writes a malformed entry. Other agents (Claude Code, Codex, VS Code) ignore unknown fields, so a per-agent `source` is safe even when other agents don't need it.

### Design

Make the per-agent shape data-driven via the YAML, not via Rust special-cases.

1. **Detector** (`crates/mirdan/src/status.rs`):
   - Extend `mcp_server_installed` (and its public callers in the status layer) to accept the agent's configured `servers_key` and probe **that** key first, then fall back to the hardcoded `["mcpServers", "servers", "mcp_servers"]` list for safety (so legacy installs still detect).
   - The call site is `component_path` → `detect_component(Component::Mcp, ...)` in `status.rs`. Thread the agent's `mcp_config.servers_key` through `check_component`/`detect_component`/`mcp_server_installed`. Use `code_context` `get callgraph` (inbound) on `mcp_server_installed` first to identify every caller that needs an updated signature.

2. **Writer** (`crates/mirdan/src/mcp_config.rs`):
   - Add an optional `entry_extras: BTreeMap<String, Value>` field on `McpConfigDef` in `crates/mirdan/src/agents.rs` (`#[serde(default)]` so missing field deserializes to empty). In `agents_default.yaml`, set on the `zed-ai` entry:
     ```yaml
     mcp_config:
       project_path: .zed/settings.json
       global_path: "~/.config/zed/settings.json"
       servers_key: context_servers
       entry_extras:
         source: custom
     ```
   - In `set_mcp_server_entry`, after writing the standard `{command, args, env}` shape, merge `entry_extras` into the resulting object (extras win on key collision so they can override defaults if a future agent needs it). Adjust `register_mcp_server` to accept and forward the extras from the `AgentDef`.
   - Keep `McpServerEntry` struct as-is — extras are stored at the per-agent definition level, not on the entry struct. Cleanest: `register_mcp_server(config_path, servers_key, tool_name, entry, extras)` with `extras: &BTreeMap<String, Value>`.

3. **Detector wiring**: `mcp_server_installed` now takes `(path, servers_key)`. Callers that have the `AgentDef` (everyone in `status.rs::check_component`) pass `agent.mcp_config.as_ref().map(|m| m.servers_key.as_str())`. The single-source-of-truth nature of the function for "is the sah MCP installed at this path?" is preserved.

### Files

- `crates/mirdan/src/agents.rs` — add `entry_extras: BTreeMap<String, serde_json::Value>` to `McpConfigDef` (with `#[serde(default)]`).
- `crates/mirdan/src/agents_default.yaml` — set `entry_extras: {source: custom}` on `zed-ai` only.
- `crates/mirdan/src/status.rs` — thread `servers_key` into `mcp_server_installed`, update call chain.
- `crates/mirdan/src/mcp_config.rs` — `register_mcp_server` accepts and merges `entry_extras`; `set_mcp_server_entry` writes them.
- Install call site(s) — wherever `register_mcp_server` is invoked from `sah init`, forward `agent.mcp_config.entry_extras` (audit via `get callgraph` inbound).

## Acceptance Criteria

- [ ] `McpConfigDef.entry_extras: BTreeMap<String, serde_json::Value>` exists with `#[serde(default)]` so older configs without the field still parse.
- [ ] `agents_default.yaml` sets `entry_extras: {source: custom}` on exactly the `zed-ai` entry — no other agent.
- [ ] `mcp_server_installed` probes the agent's configured `servers_key` first when one is known, then falls back to the hardcoded list. For Zed, an entry under `context_servers.sah` with `command: "sah"` is detected as installed.
- [ ] `register_mcp_server` writes Zed entries with `source: "custom"` alongside `command`, `args`, `env`. The `set_mcp_server_entry` merge logic favors extras when keys collide (so YAML can override defaults if a future agent needs it).
- [ ] Existing agents (Claude Code, Codex, etc.) keep writing their current shape — no `source` field appears in their entries.
- [ ] On a machine with Zed installed and a hand-written JSONC `~/.config/zed/settings.json`:
  - `sah init user` writes a valid `context_servers.sah` entry that Zed loads at startup.
  - `sah doctor` reports `Zed AI · user · MCP server: Ok found at ...` (no longer warning).
- [ ] The current scope-pair policy still holds: a project-scope MCP missing with user-scope installed demotes to `Ok`.

## Tests

- [ ] `crates/mirdan/src/status.rs::tests::test_mcp_installed_respects_servers_key` — synthetic agent with `servers_key: context_servers`, write a settings.json with `{"context_servers": {"sah": {"command": "sah", "source": "custom"}}}`, assert `mcp_server_installed` returns `true`.
- [ ] `crates/mirdan/src/status.rs::tests::test_mcp_installed_falls_back_to_default_keys` — no `servers_key` provided, settings.json uses `mcpServers` (legacy/JSON default): still detected.
- [ ] `crates/mirdan/src/mcp_config.rs::tests::test_register_mcp_server_writes_entry_extras` — call `register_mcp_server` with `entry_extras: {source: custom}`, read the resulting JSON, assert `["context_servers"]["sah"]["source"] == "custom"` and that `command`/`args` are still present.
- [ ] `crates/mirdan/src/mcp_config.rs::tests::test_register_mcp_server_preserves_other_agents_shape` — entry_extras empty, output has no `source` key.
- [ ] `crates/mirdan/src/mcp_config.rs::tests::test_zed_install_against_jsonc_settings` — end-to-end: write a JSONC settings.json with comments, run the Zed install path, read the file back as JSONC, assert the `context_servers.sah` entry with `source: "custom"` is present and the file still parses.
- [ ] Gates: `cargo test -p mirdan`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo fmt --all -- --check`.

## Out of scope

- Preserving comments when rewriting JSONC files (a separate UX concern — rewriting strips comments today; user accepts this for now or a follow-up card handles it via a JSONC-preserving writer).
- The "Zed multi-project drops `.zed/settings.json`" bug (zed-industries/zed#51951) — Zed-side, not ours.
- Extending `entry_extras` to other agents that don't need it.

## Workflow

`/tdd`. Write the five tests first; they all fail (detector can't see `context_servers`, writer doesn't emit `source`). Then implement the YAML schema change, the detector signature change with backward-compat fallback, and the writer extras merge.

## Depends on

- 01KSNANRDMEDCD5N8VDWAF4JHF (JSONC reading must be in place — Zed's settings.json is JSONC) #init-doctor