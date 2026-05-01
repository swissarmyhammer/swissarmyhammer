---
depends_on:
- 01KKC8ASRCG1CKVV8RPG74YADK
position_column: done
position_ordinal: ffffffffffff8380
title: 'STATUSLINE-6: Pipeline + CLI integration (statusline + statusline config)'
---
## What
Wire the full pipeline in `lib.rs` and add two CLI subcommands: `sah statusline` (main hook) and `sah statusline config` (dump annotated config).

Key files:
- `swissarmyhammer-statusline/src/lib.rs` — `pub fn run()` pipeline + `pub fn dump_config()`
- `swissarmyhammer-cli/Cargo.toml` — add `swissarmyhammer-statusline` dependency
- `swissarmyhammer-cli/src/cli.rs` — add `Statusline` variant to `Commands` with `Config` subcommand
- `swissarmyhammer-cli/src/main.rs` — add routing for both subcommands

### `sah statusline` (main hook)
Pipeline in `run()`:
1. `stdin().read_to_string()` — read all JSON
2. `serde_json::from_str::<StatuslineInput>()` — parse
3. `load_statusline_config()` — stacked YAML
4. `parse_format(config.format)` — ordered segment list
5. `build_registry()` — module function map
6. For each FormatSegment: if Module, call module_fn with its config section, apply per-module format string, style result
7. `print!(\"{}\", output)` — no trailing newline

The cwd for tool modules comes from `input.workspace.current_dir` falling back to `std::env::current_dir()`.

### `sah statusline config` (dump config)
`dump_config()` prints the raw builtin `config.yaml` content (with all inline comments) to stdout. Users redirect: `sah statusline config > .swissarmyhammer/statusline/config.yaml`

This follows starship's `starship print-config` pattern — gives users a fully commented starting point.

## Acceptance Criteria
- [ ] `echo '{...}' | sah statusline` produces styled output
- [ ] Empty JSON input produces graceful fallback (no crash)
- [ ] Modules that return None are silently skipped
- [ ] `sah statusline` appears in `sah --help`
- [ ] `sah statusline config` dumps the full annotated builtin YAML
- [ ] Exit code 0 on success, non-zero on parse errors

## Tests
- [ ] Integration test: pipe full JSON, verify output contains expected segments
- [ ] Integration test: pipe minimal JSON, verify no crash
- [ ] Integration test: pipe invalid JSON, verify non-zero exit
- [ ] Unit test: dump_config() returns non-empty valid YAML
- [ ] CLI test: `sah statusline --help` works
- [ ] `cargo test -p swissarmyhammer-statusline && cargo test -p swissarmyhammer-cli`"