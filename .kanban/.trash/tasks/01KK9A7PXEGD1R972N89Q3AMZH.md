---
position_column: todo
position_ordinal: f5
title: 'CODE-CONTEXT-0: Implement cargo run -- doctor command'
---
Expose the doctor diagnostic check as a CLI command that validates LSP availability.

**Requirements:**
- Add `cargo run -- doctor` CLI command to swissarmyhammer-cli
- Call doctor::run_doctor() on the working directory (or specified path)
- Output project type and LSP availability in human-readable format
- Show which LSP servers are installed (rust-analyzer, etc.)
- Show paths to installed LSP binaries

**Quality Test Criteria:**
1. Build succeeds: `cargo build 2>&1 | grep -c "error"` = 0
2. Command works: `cargo run -- doctor` on any Rust project outputs project type
3. LSP detection works on swissarmyhammer-tools:
   - Detects project_type = "rust"
   - Detects rust-analyzer as installed=true
   - Shows path to rust-analyzer binary
4. Output format is clear and readable (not JSON, human text)
5. Works on non-Rust projects (shows project_type=None or appropriate type)
6. No crashes on invalid/missing paths
7. Integration test validates doctor output on real project

**Example Output:**
```
Project Type: rust
LSP Servers:
  ✓ rust-analyzer (installed) at /usr/local/bin/rust-analyzer
```