---
assignees:
- claude-code
depends_on:
- 01KNS1VVN8B6P7CFXGD38PE3MD
- 01KNS1X4Y4M5R6K9GJVFPFPVEX
- 01KNS1XMZFNHGWMGKNSHZS8TM2
- 01KNS1Y49EX2CZJQ38WGM23954
- 01KNS1YN0KDTXK1ACA1NBS48XJ
- 01KNS1Z702R8FS8QQMA0J3MTB2
position_column: todo
position_ordinal: '8980'
project: code-context-cli
title: Wire up main.rs — tracing, dispatch, and banner
---
## What
Complete `code-context-cli/src/main.rs` by wiring together all modules, mirroring `shelltool-cli/src/main.rs` exactly.

```rust
mod banner;
mod cli;
mod doctor;
mod ops;
mod registry;
mod serve;
mod skill;
```

### Key imports (derived from shelltool main.rs):
```rust
use clap::Parser;
use std::sync::{Arc, Mutex};  // std::sync, NOT tokio — for file logging
use swissarmyhammer_common::lifecycle::{InitRegistry, InitScope};
use swissarmyhammer_common::reporter::CliReporter;
use swissarmyhammer_directory::{DirectoryConfig, CodeContextConfig};  // NOT ShellConfig
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};
```

### `FileWriterGuard` struct:
Wraps `Arc<Mutex<std::fs::File>>` (std::sync::Mutex, not tokio) for flush-on-every-write logging. Copy exactly from shelltool.

### Log directory:
```rust
let log_dir = std::path::PathBuf::from(CodeContextConfig::DIR_NAME);
// → creates `.code-context/mcp.log`
```
**Critical**: shelltool uses `ShellConfig::DIR_NAME` (= `".shell"`). Code-context uses `CodeContextConfig::DIR_NAME` (= `".code-context"`). Both are in `swissarmyhammer-directory/src/config.rs`.

### Debug filter:
```rust
"code_context_cli=debug,swissarmyhammer_tools=debug,swissarmyhammer_code_context=debug"
```
(shelltool uses `"shelltool=debug,swissarmyhammer_tools=debug,swissarmyhammer_shell=debug"`)

### Banner:
- Call `banner::should_show_banner(&args)` before `Cli::parse()`
- If true, call `banner::print_banner()`

### `dispatch_command(cli: Cli) -> i32` (async):
```rust
match cli.command {
    Commands::Serve => match serve::run_serve().await {
        Ok(()) => 0,
        Err(e) => { eprintln!("Error: {}", e); 1 }
    },
    Commands::Init { target } => {
        let scope = match target {
            InstallTarget::Project => InitScope::Project,
            InstallTarget::Local => InitScope::Local,
            InstallTarget::User => InitScope::User,
        };
        let mut reg = InitRegistry::new();
        registry::register_all(&mut reg);
        let reporter = CliReporter;
        let results = reg.run_all_init(&scope, &reporter);
        if results.iter().any(|r| r.status == InitStatus::Error) { 1 } else { 0 }
    }
    Commands::Deinit { target } => { /* mirror Init with run_all_deinit */ }
    Commands::Doctor { verbose } => doctor::run_doctor(verbose),
    Commands::Skill => skill::run_skill(),
    // All operation variants:
    Commands::Get(_) | Commands::Search(_) | Commands::List(_)
    | Commands::Grep(_) | Commands::Query(_) | Commands::Find(_)
    | Commands::Build(_) | Commands::Clear(_) | Commands::Lsp(_)
    | Commands::Detect(_) => ops::run_operation(&cli.command, cli.json).await,
}
```

## Acceptance Criteria
- [ ] `cargo build -p code-context-cli` succeeds
- [ ] `./target/debug/code-context --help` shows the banner and help text
- [ ] `./target/debug/code-context doctor` runs and exits 0/1/2
- [ ] `./target/debug/code-context get status` runs and exits cleanly
- [ ] Log file created at `.code-context/mcp.log` when `serve` runs

## Tests
- [ ] `test_dispatch_serve_compiles` — `dispatch_command` is async and matches all variants
- [ ] Run `cargo test -p code-context-cli` — all tests in all modules pass

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.