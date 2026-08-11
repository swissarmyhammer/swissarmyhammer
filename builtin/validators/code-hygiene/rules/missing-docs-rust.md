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
    set -e
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    cargo clippy --workspace --message-format=json --quiet -- -W missing_docs > "$work/clippy.json"
    jq -c 'select(.reason == "compiler-message")
           | .message
           | select(.code.code == "missing_docs")
           | select(.spans | length > 0)
           | {file: .spans[0].file_name, line: .spans[0].line_start, message: .message}
           | select(.file | startswith("/") | not)' "$work/clippy.json" |
      sort -u
  doctor:
    check_command: "which cargo-clippy jq sort mktemp"
    check_version_command: "cargo clippy --version"
    fix_hint: "rustup component add clippy"
---

# Missing Documentation — Rust

`cargo clippy` reports every public item without documentation. `missing_docs`
is a rustc lint, and it is off by default. The `-W missing_docs` flag turns it
on, so the rule owns its own lint level and never reads the crate's own lint
attributes for this check.

The scope is `workspace` because cargo lints a package, never a loose file.
The engine keeps only the findings in the changed files.

Every measurement below was made with clippy 0.1.97.

## `--workspace` lints every member, and the scope declares that

Measured on a probe workspace that holds three packages: a root package, a
`shared` member the root package names as both a dependency and a
build-dependency, and a `lonely` member no package names. Each of the three
holds one undocumented `pub struct`.

| command | files reported |
|---|---|
| without `--workspace` | `src/lib.rs`, `shared/src/lib.rs` |
| with `--workspace` | `src/lib.rs`, `shared/src/lib.rs`, `lonely/src/lib.rs` |

Cargo builds a package the working directory names, and it builds the packages
that package depends on. It builds no member nothing depends on. `--workspace`
selects every member, so the command then reads the whole workspace the
`scope: workspace` declaration states. The acceptance test
`the_shipped_rust_missing_docs_tool_rule_reports_every_workspace_member` holds
the run to all three files.

The command takes no `--all-targets`. Measured on a probe crate that holds
`tests/it.rs` with an undocumented `pub struct IntegrationHelper`: the shipped
command stays silent about that file, and the same command with
`--all-targets` reports it. A test target is therefore outside this rule.

## `sort -u` collapses a repeated finding

Cargo compiles a package one time for each way the workspace uses it, and
clippy writes the diagnostics of each compilation. Measured on the probe
workspace above: `shared` is a dependency and a build-dependency of the root
package, so `shared/src/lib.rs:3` arrives two times. `sort -u` leaves one.

Measured on this workspace: the run reports 1262 lines, and `sort -u` leaves
1179. The 83 lines it removes repeat 77 findings, which stand in 18 files.

## Generated code, which the prompt rule carves out

The `missing-docs` prompt rule carves out generated code. Cargo writes
generated code under `OUT_DIR`, and a crate reads it with an `include!`. The
diagnostic then carries the absolute path of the generated file, at a file the
author cannot edit.

Measured on a probe crate whose build script writes one undocumented
`pub struct` and one undocumented `pub fn` into `OUT_DIR`, and whose library
`include!`s that file and holds one undocumented `pub struct` of its own: the
run without the `select` step reports 3 findings, two of them at an absolute
path under `target/`; the run with the step reports the one hand-written item.
`select(.file | startswith("/") | not)` is the step that drops them, and the two
Rust rules beside this one carry the same step. The acceptance test
`the_shipped_rust_missing_docs_tool_rule_names_no_generated_file` holds the
script to naming neither generated item.

The engine drops such a finding as well, because it keeps only the findings in
the changed files and no changed file stands under `target/`. The step is what
makes the SCRIPT's own answer equal the rule's answer, so a person who runs the
script by hand reads the same list the review reads.

## A run cannot answer zero for a broken tool

`cargo clippy` exits 101 for a crate that does not compile, and it writes no
`missing_docs` diagnostic for that crate. A shell pipeline takes the exit
status of its LAST command, and that command was `jq`, so the earlier pipe
exited 0 and reported nothing. That reads exactly like a clean crate.

The script now writes cargo's report to a file rather than into a pipe, and
`set -e` makes cargo's own exit status the exit status of the script. Measured
over a crate whose library holds a `pub struct Undocumented` with no closing
semicolon: the earlier pipe reported no finding and exited 0; the script
reports no finding and exits 101, with cargo's own error on stderr. The
acceptance test
`the_shipped_rust_missing_docs_tool_rule_breaks_on_a_crate_that_does_not_compile`
holds that behaviour.

The `trap` removes the temporary directory when the script exits.

## What rustc carves out for itself

Measured on one probe crate that holds each shape below. The run reports 2
findings, and both of them stand in the section under this one. Each shape
here reports nothing.

- **A private item.** `struct PrivateStruct` and `fn private_function` report
  nothing. A `pub struct` and a `pub fn` inside a private `mod` report nothing
  either, because no path outside the crate reaches them. This is the prompt
  rule's private carve-out, reproduced by the language.
- **A test.** A `#[cfg(test)] mod tests` that holds `pub struct
  TestHelperType`, `pub fn test_helper` and a `#[test] fn` reports nothing. Two
  facts each carry it: the module is private, and `#[cfg(test)]` compiles into
  no target this command builds.
- **An obvious implementation.** `impl fmt::Display for Shown` and
  `impl Default for Shown` report nothing. `missing_docs` asks for
  documentation on the trait item, never on the `impl` that answers it. This is
  the prompt rule's "Obvious implementations (Display, Debug, ToString, etc.)"
  carve-out, reproduced by the compiler.
- **`#[doc(hidden)]`.** A `#[doc(hidden)] pub struct` and a `#[doc(hidden)]
  pub fn` report nothing.

## What rustc does not carve out

The `missing-docs` prompt rule carves out "Simple getters/setters with
self-explanatory names". `missing_docs` has no setting for it. Measured on the
same probe crate: the undocumented `pub fn value(&self) -> i32` and the
undocumented `pub fn set_value(&mut self, next: i32)` beside it are the 2
findings of that run.

So a public getter and a public setter each need a doc comment. The recourse is
the inline suppression at the end of this file.

The lint also asks each compiled target for its own crate documentation.
Measured: a `build.rs` with no `//!` comment reports `missing documentation for
the crate` at line 1. A build script is a file the author writes, so that
finding stands.

## How to exempt one item

The `jq` filter selects the `missing_docs` diagnostics and drops every other
lint clippy emits. Selection here is attribution, not exemption: to exempt one
item, write `#[allow(missing_docs)]` on it in the code. Measured: the
annotation on the undocumented getter above leaves the setter as the only
finding of the run.

## The rule declares no install commands

Clippy is a component of the Rust toolchain, not a package with its own
version, so no install command can pin it. The `doctor.fix_hint` states
`rustup component add clippy` instead, which installs it for the toolchain the
project already uses. `sah doctor` shows that hint as the fix; the install
lifecycle never runs it.
