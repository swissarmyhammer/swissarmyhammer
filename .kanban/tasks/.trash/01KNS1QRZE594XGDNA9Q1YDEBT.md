---
assignees:
- claude-code
depends_on: []
position_column: todo
position_ordinal: b080
project: kanban-mcp
title: 'kanban-cli: add build.rs for man pages, shell completions, and doc reference'
---
## What

Create `kanban-cli/build.rs` to generate CLI docs, man pages, and shell completions from the new `cli.rs` at build time.

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
        &cmd,
        &repo_root.join("doc/src/reference"),
        "kanban",
        Some("swissarmyhammer/tap/kanban-cli"),
    )?;

    doc_gen::generate_manpage(&cmd, &repo_root.join("docs"), "kanban")?;

    doc_gen::generate_completions(cmd, &repo_root.join("completions"), "kanban")?;

    Ok(())
}
```

This produces:
- `doc/src/reference/kanban-cli.md` — mdbook CLI reference
- `docs/kanban.1` — man page
- `completions/kanban.bash`, `completions/kanban.fish` — shell completions

Note: `build.rs` only generates docs for the structured `cli::Cli` subcommands (serve/init/deinit/doctor). The schema-driven noun-verb commands are not covered — that's consistent with how shelltool works (only its lifecycle commands are in `cli.rs`).

## Acceptance Criteria
- [ ] `cargo build -p kanban-cli` generates `doc/src/reference/kanban-cli.md`
- [ ] `completions/kanban.bash` and `completions/kanban.fish` are created
- [ ] Man page `docs/kanban.1` is created
- [ ] The generated markdown lists serve/init/deinit/doctor subcommands

## Tests
- [ ] `cargo build -p kanban-cli` succeeds without errors
- [ ] Assert generated files exist after build

## Workflow
- Use `/tdd` — write failing tests first, then implement to make them pass.
