---
assignees:
- claude-code
depends_on:
- 01KP0KZZ9VDQJVNK15JQAY4BKH
position_column: todo
position_ordinal: b280
project: kanban-mcp
title: 'kanban-cli: add build.rs for man pages, shell completions, and doc reference'
---
## What

Create `kanban-cli/build.rs` to generate CLI docs, man pages, and shell completions at build time.

Model exactly on `shelltool-cli/build.rs`:

```rust
#[path = "src/cli.rs"]
mod cli;

#[path = "../build-support/doc_gen.rs"]
mod doc_gen;

fn main() -> std::io::Result<()> {
    let cmd = cli::Cli::command();
    let repo_root = Path::new("..");

    doc_gen::generate_markdown_with_brew(
        &cmd, &repo_root.join("doc/src/reference"), "kanban",
        Some("swissarmyhammer/tap/kanban-cli"),
    )?;
    doc_gen::generate_manpage(&cmd, &repo_root.join("docs"), "kanban")?;
    doc_gen::generate_completions(cmd, &repo_root.join("completions"), "kanban")?;
    Ok(())
}
```

This generates docs for the lifecycle subcommands (serve/init/deinit/doctor) defined in `cli.rs`. The schema-driven noun/verb commands are not covered — same trade-off as shelltool (only its clap-defined commands appear in generated docs).

Add `[build-dependencies]` to `kanban-cli/Cargo.toml`:
- `clap` (workspace)
- `clap-markdown` (workspace)
- `clap_mangen` (workspace)
- `clap_complete` (workspace)

## Acceptance Criteria
- [ ] `cargo build -p kanban-cli` generates `doc/src/reference/kanban-cli.md`
- [ ] `completions/kanban.bash` and `completions/kanban.fish` are created
- [ ] Man page `docs/kanban.1` is created
- [ ] The generated markdown lists serve/init/deinit/doctor subcommands

## Tests
- [ ] `cargo build -p kanban-cli` succeeds without errors
- [ ] Assert generated files exist after build
