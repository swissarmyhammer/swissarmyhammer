---
assignees:
- claude-code
position_column: todo
position_ordinal: ba80
project: kanban-mcp
title: 'sah-cli: add build.rs, retire generate_docs.rs binary'
---
## What

Create `swissarmyhammer-cli/build.rs` for man pages, shell completions, and doc reference generation at build time, matching shelltool-cli/build.rs and code-context-cli/build.rs. Retire `generate_docs.rs` as a separate binary — the build.rs approach is the consistent pattern.

## Acceptance Criteria
- [ ] `swissarmyhammer-cli/build.rs` exists using `build-support/doc_gen.rs`
- [ ] `generate_docs.rs` binary removed or deprecated
- [ ] `cargo build -p swissarmyhammer-cli` generates docs, man page, completions
- [ ] `[build-dependencies]` added: clap, clap-markdown, clap_mangen, clap_complete
