---
position_column: done
position_ordinal: '9280'
title: Create kanban-cli crate with banner and workspace registration
---
## What
Create new `kanban-cli/` crate that produces a `kanban` binary.

**Files to create:**
- `kanban-cli/Cargo.toml` — `[[bin]] name = "kanban"`, lightweight deps: swissarmyhammer-kanban, swissarmyhammer-operations, clap, tokio, serde_json, urlencoding, tracing, tracing-subscriber, open = "5"
- `kanban-cli/src/banner.rs` — Blue/cyan ANSI 256-color gradient kanban board ASCII art + block "KANBAN" text. Same structure as `swissarmyhammer-cli/src/banner.rs`: COLORS[19], LOGO[19], `should_show_banner()`, `print_banner()`, `render_banner()`

**Files to modify:**
- `Cargo.toml` (workspace root) — add `"kanban-cli"` to members list

## Acceptance Criteria
- [ ] `cargo build -p kanban-cli` produces `target/debug/kanban` binary
- [ ] `kanban --help` shows a colored banner in terminal
- [ ] No binary name conflict with kanban-app

## Tests
- [ ] Banner unit tests: `should_show_banner` logic, plain vs colored rendering, LOGO/COLORS length match
- [ ] `cargo nextest run -p kanban-cli` passes