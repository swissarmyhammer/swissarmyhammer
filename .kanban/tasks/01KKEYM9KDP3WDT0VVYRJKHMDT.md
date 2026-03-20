---
position_column: done
position_ordinal: d380
title: 'Add unit tests for instructions formatting: missing LSP, all present, no projects'
---
## What

Expand the `instructions` field in `InitializeResult` (returned during MCP handshake) to include a health summary of detected projects and LSP server availability. This surfaces missing LSP servers to the user at the very start of every Claude Code session without any action required.

**Files to modify:**
- `swissarmyhammer-tools/src/mcp/server.rs` — the `initialize()` handler in `ServerHandler` impl (~line 1166)
- `swissarmyhammer-tools/src/mcp/tools/code_context/doctor.rs` — reuse `run_doctor()` or `detect_project_types()` + `get_lsp_servers_for_type()`

**Approach:**
1. In `initialize()`, after `initialize_code_context()`, run the doctor check against `self.work_dir`
2. Build a status string from the `DoctorReport`: list detected project types and for each LSP server, whether it's available or missing (with install hint)
3. Append this to `SERVER_INSTRUCTIONS` to form the final `instructions` string
4. Only append if there are missing LSP servers (don't add noise when everything is fine)

**Example output in instructions:**
```
The only coding assistant you'll ever need. Write specs, not code.

setupStatus: Detected projects: rust, javascript
  rust-analyzer: installed
  typescript-language-server: NOT FOUND — install with: npm install -g typescript-language-server typescript
```

**Considerations:**
- `run_doctor()` calls `which` and `--version` synchronously — this adds latency to the MCP handshake. Keep it fast by only checking `which` (skip `--version` for this path) or accept the small delay.
- The `doctor.rs` functions are currently in `swissarmyhammer-tools` so they're directly accessible from `server.rs`.

## Acceptance Criteria
- [ ] When a user starts a Claude Code session with sah, and an LSP server is missing, the MCP instructions include a notice with the install command
- [ ] When all LSP servers are present, the instructions do NOT include extra setup status (no noise)
- [ ] The handshake latency increase is under 200ms

## Tests
- [ ] Unit test in `server.rs`: mock a workspace with Cargo.toml, verify instructions contain "rust-analyzer" info when binary is missing
- [ ] Unit test: verify instructions are unchanged (just SERVER_INSTRUCTIONS) when no projects detected
- [ ] `cargo test -p swissarmyhammer-tools` passes