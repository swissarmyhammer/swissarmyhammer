---
assignees:
- claude-code
position_column: todo
position_ordinal: '80'
project: code-context-cli
title: Create code-context-cli crate scaffolding
---
## What
Create the `code-context-cli` crate in the workspace, mirroring `shelltool-cli` structure exactly.

### Files to create:

**`code-context-cli/Cargo.toml`**:
```toml
[package]
name = "code-context-cli"
version.workspace = true
edition.workspace = true
license.workspace = true
repository.workspace = true
homepage.workspace = true
description = "Standalone MCP code-context tool CLI for AI coding agents"

[package.metadata.dist]
formula = "code-context"

[[bin]]
name = "code-context"
path = "src/main.rs"

[dependencies]
swissarmyhammer-tools = { workspace = true }
swissarmyhammer-common = { workspace = true }
swissarmyhammer-directory = { workspace = true }
swissarmyhammer-doctor = { workspace = true }
swissarmyhammer-config = { workspace = true }
swissarmyhammer-code-context = { workspace = true }
swissarmyhammer-skills = { workspace = true }
swissarmyhammer-templating = { workspace = true }
swissarmyhammer-lsp = { workspace = true }
swissarmyhammer-project-detection = { workspace = true }
mirdan = { workspace = true }
rmcp = { workspace = true }
clap = { workspace = true }
tokio = { workspace = true }
tracing = { workspace = true }
tracing-subscriber = { workspace = true }
serde_json = { workspace = true }
dirs = { workspace = true }
tempfile = { workspace = true }
async-trait = { workspace = true }

[build-dependencies]
clap = { workspace = true }
clap-markdown = { workspace = true }
clap_mangen = { workspace = true }
clap_complete = { workspace = true }
```

Note the additional deps vs. original plan:
- `swissarmyhammer-skills` + `swissarmyhammer-templating` + `tempfile` — needed by `skill.rs` for `SkillResolver`, template rendering, and tempdir-based deploy
- `swissarmyhammer-lsp` + `swissarmyhammer-project-detection` — needed by `doctor.rs` for LSP status checks
- `async-trait` — needed by `serve.rs` for `ServerHandler` impl

The `[package.metadata.dist]` section opts this crate into the cargo-dist pipeline — Homebrew formula, shell installer, and GitHub Actions release CI are all auto-configured via `dist-workspace.toml`. No changes to `dist-workspace.toml` or `.github/workflows/release.yml` are needed.

**`code-context-cli/src/main.rs`** — stub:
```rust
mod banner;
mod cli;
mod doctor;
mod ops;
mod registry;
mod serve;
mod skill;

#[tokio::main]
async fn main() {}
```

**`code-context-cli/build.rs`**:
```rust
use std::path::Path;
use clap::CommandFactory;

#[path = "src/cli.rs"]
mod cli;

#[path = "../build-support/doc_gen.rs"]
mod doc_gen;

fn main() -> std::io::Result<()> {
    let cmd = cli::Cli::command();
    let repo_root = Path::new("..");

    doc_gen::generate_markdown_with_brew(
        &cmd,
        &repo_root.join("doc/src/reference"),
        "code-context",
        Some("swissarmyhammer/tap/code-context-cli"),
    )?;

    doc_gen::generate_manpage(&cmd, &repo_root.join("docs"), "code-context")?;

    doc_gen::generate_completions(cmd, &repo_root.join("completions"), "code-context")?;

    Ok(())
}
```

**Register in root `Cargo.toml`** — add `"code-context-cli"` to `[workspace] members` list after `"shelltool-cli"`.

## Acceptance Criteria
- [ ] `cargo check -p code-context-cli` passes with no errors
- [ ] `cargo metadata --no-deps --format-version 1 | grep code-context-cli` confirms crate in workspace
- [ ] `[package.metadata.dist]` present with `formula = "code-context"`
- [ ] build.rs references `build-support/doc_gen.rs` with binary name `"code-context"`

## Tests
- [ ] `cargo test -p code-context-cli` passes (stub, just verifies compilation)
- [ ] Run `cargo check -p code-context-cli` and confirm clean

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.