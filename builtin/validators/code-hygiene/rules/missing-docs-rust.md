---
name: missing-docs-rust
description: Public Rust items need docs — checked by clippy, not by prompt.
match:
  files:
    - "**/*.rs"
  project_types:
    - rust
supersedes: missing-docs
tool:
  scope: workspace
  run: |
    cargo clippy --message-format=json --quiet -- -W missing_docs |
      jq -c 'select(.reason == "compiler-message")
             | .message
             | select(.code.code == "missing_docs")
             | select(.spans | length > 0)
             | {file: .spans[0].file_name, line: .spans[0].line_start, message: .message}'
  doctor:
    check_command: "which cargo-clippy jq"
    check_version_command: "cargo clippy --version"
---

# Missing Documentation — Rust

`cargo clippy` reports every public item without documentation. The `-W
missing_docs` flag turns on the lint for the workspace members, so the rule
owns its own lint level and never reads the crate's own lint attributes for
this check.

The scope is `workspace` because cargo lints a package, never a loose file.
The engine keeps only the findings in the changed files.

The `jq` filter selects the `missing_docs` diagnostics and drops every other
lint clippy emits. Selection here is attribution, not exemption: to exempt one
item, write `#[allow(missing_docs)]` on it in the code.

The rule declares no install commands. Clippy is a component of the Rust
toolchain, not a package with its own version, so `rustup component add
clippy` installs it for the toolchain the project already uses.
