---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffff8880
title: 'shelltool-cli crate: scaffold and serve command'
---
Create the shelltool-cli crate with the MCP server.\n\n## Structure\n- shelltool-cli/Cargo.toml — [[bin]] name = \"shelltool\", [package.metadata.dist] formula = \"shelltool\", workspace fields, deps on swissarmyhammer-tools, swissarmyhammer-shell, swissarmyhammer-common, rmcp, clap, tokio, tracing\n- shelltool-cli/src/main.rs — clap dispatch, banner on interactive help\n- shelltool-cli/src/cli.rs — Cli struct with Commands: Serve, Init, Deinit, Doctor\n- shelltool-cli/src/serve.rs — minimal stdio MCP server hosting only ShellExecuteTool via rmcp\n\n## Also\n- Add \"shelltool-cli\" to root Cargo.toml workspace members\n- build.rs using build-support/doc_gen.rs for markdown, manpages, completions\n- Build deps: clap, clap-markdown, clap_mangen, clap_complete\n\n## Acceptance\n- `cargo build -p shelltool-cli` succeeds\n- `shelltool serve` starts MCP stdio server with shell tool\n- `shelltool --help` shows all subcommands