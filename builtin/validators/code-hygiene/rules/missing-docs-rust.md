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
    status=0
    cargo clippy --workspace --message-format=json --quiet -- -W missing_docs \
      > "$work/clippy.json" || status=$?
    filtered=0
    jq -r 'select(.reason == "build-finished") | "ran"' "$work/clippy.json" \
      > "$work/ran.txt" || filtered=$?
    jq -r 'select(.reason == "compiler-message")
           | .message
           | select(.level == "error")
           | (.code.code // "")
           | select(. == "" or test("^E[0-9]+$"))' "$work/clippy.json" \
      > "$work/rustc-errors.txt" || filtered=$?
    jq -r 'select(.reason == "compiler-message")
           | .message
           | select(.level == "error")
           | "an error"' "$work/clippy.json" \
      > "$work/errors.txt" || filtered=$?
    jq -s -r '[.[] | select(.reason == "compiler-artifact")
                   | select(.target.kind | index("custom-build"))
                   | .package_id]
              - [.[] | select(.reason == "build-script-executed") | .package_id]
              | .[]' "$work/clippy.json" \
      > "$work/unrun-build-scripts.txt" || filtered=$?
    jq -c 'select(.reason == "compiler-message")
           | .message
           | select(.code.code == "missing_docs")
           | select(.spans | length > 0)
           | {file: .spans[0].file_name, line: .spans[0].line_start, message: .message}
           | select(.file | startswith("/") | not)' "$work/clippy.json" \
      > "$work/findings.json" || filtered=$?
    if [ "$filtered" -ne 0 ]; then
      printf 'missing-docs-rust: jq could not read the clippy report\n' >&2
      exit 1
    fi
    if [ "$status" -ne 0 ] && [ ! -s "$work/ran.txt" ]; then
      printf 'missing-docs-rust: cargo could not run clippy over the workspace\n' >&2
      exit 1
    fi
    if [ -s "$work/rustc-errors.txt" ]; then
      printf 'missing-docs-rust: cargo clippy could not lint the workspace\n' >&2
      exit 1
    fi
    if [ -s "$work/unrun-build-scripts.txt" ]; then
      printf 'missing-docs-rust: a build script did not run, so clippy did not lint every crate\n' >&2
      exit 1
    fi
    if [ "$status" -ne 0 ] && [ ! -s "$work/errors.txt" ]; then
      printf 'missing-docs-rust: cargo stopped the build and wrote no compiler error\n' >&2
      exit 1
    fi
    sort -u "$work/findings.json"
  doctor:
    check_command: "which cargo-clippy jq sort mktemp"
    check_version_command: "cargo clippy --version"
    fix_hint: "rustup component add clippy"
---

# Missing Documentation — Rust

`cargo clippy` reports a public item with no documentation. It stays silent
about a private item, about an item inside a private `mod`, about a
`#[cfg(test)]` item, about a trait `impl` item and about a `#[doc(hidden)]`
item. Measured on one probe crate that holds each shape: the run reports 2
findings, the undocumented `pub fn value` and the undocumented
`pub fn set_value`. The section "What rustc carves out for itself" below states
each measurement.

`missing_docs` is a rustc lint, and it is off by default. The `-W missing_docs`
flag turns it on, so the rule owns its own lint level and never reads the
crate's own lint attributes for this check.

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

Cargo compiles a package one time for each way the workspace names it, and
clippy writes the diagnostics of each compilation. Measured on the probe
workspace above, where the root package names `shared` as a dependency and as a
build-dependency: cargo compiles `shared` two times, and `shared/src/lib.rs:3`
arrives two times. `sort -u` leaves one.

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

## A workspace the tool cannot lint

`cargo clippy` exits nonzero for four different reasons, and one status carries
all four:

- cargo could not start a run at all.
- cargo made a run, and a crate failed its type check. Clippy runs `missing_docs`
  AFTER that type check, so it never linted that crate.
- cargo made a run, and the BUILD SCRIPT of a crate broke. cargo runs a build
  script before it compiles the crate that script serves, so clippy never
  linted that crate. This repository holds fifteen build scripts.
- cargo made a run, clippy linted every crate, and a lint stands at deny level.

The first three are broken runs. The fourth is a MEASURED run, and the findings
it holds must stand. `builtin/validators/README.md` states the answer for this
shape: "One status can carry both a measured run and a broken run. The status of
a failure is then the same as the status of a finding. The script must then test
the REPORT beside the status, and accept the shared status only for the report
shape a measured run writes." The two Rust rules beside this one, `dead-code-rust`
and `complexity-rust`, make the same four tests over the same cargo report.

The deny-level shape is not a corner case for THIS rule. Under
`RUSTFLAGS="-D warnings"` a `missing_docs` diagnostic itself arrives at level
`error` and cargo exits 101, so a gate that read the status alone would break
the run exactly when the rule has a finding. Measured with clippy 0.1.97 over
one package holding one undocumented `pub struct`: the raw report holds the
error code `missing_docs`, the filter selects on the CODE and keeps the finding,
and the run answers 1 finding at exit 0.

The FILTERED findings answer none of the four. One member that compiles fills
that file while another member never reached the lint, so a workspace with a
broken member reads as a clean tree. The script therefore reads the RAW report,
which carries what the filter drops, and breaks the run in four places:

- the status is nonzero and the raw report holds no `build-finished` entry —
  `missing-docs-rust: cargo could not run clippy over the workspace`;
- the raw report holds an error-level message with a rustc code or with no code
  — `missing-docs-rust: cargo clippy could not lint the workspace`;
- the raw report holds a `custom-build` artifact whose package writes no
  `build-script-executed` entry —
  `missing-docs-rust: a build script did not run, so clippy did not lint every crate`;
- the status is nonzero and the raw report holds no error-level message at all
  — `missing-docs-rust: cargo stopped the build and wrote no compiler error`.

Each of the five `jq` calls writes to a file and tests its own status, because
the script writes `set -e` with no `pipefail` and a pipeline takes the status of
its LAST command. The earliest shape of this script ended in `sort -u`, so a
`jq` that could not run answered exit 0 with no finding, which reads exactly
like a clean tree. Measured over the healthy package below with `jq` replaced by
a command that exits 127: the pipe shape wrote 0 findings and exited 0; the
shipped shape writes 0 findings, `missing-docs-rust: jq could not read the
clippy report` on stderr, and exit 1.

Every shape below was measured with clippy 0.1.97 and cargo 1.97.1. Each package
that holds a finding holds one undocumented `pub struct Undocumented`.

| the shape | status | `build-finished` | error codes | the run answers |
|---|---|---|---|---|
| a healthy package, one undocumented item | 0 | `success: true` | none | 1 finding, exit 0 |
| a package that does not parse: `pub struct Undocumented` | 101 | `success: false` | no code | 0 findings, exit 1 |
| a workspace of two members, one that does not parse beside one undocumented item | 101 | `success: false` | no code | 0 findings, exit 1 |
| `[lints.rust] unused_variables = "deny"` beside one undocumented item | 101 | `success: false` | `unused_variables` | 1 finding, exit 0 |
| the same manifest beside no undocumented item | 101 | `success: false` | `unused_variables` | 0 findings, exit 0 |
| `RUSTFLAGS="-D warnings"` beside one undocumented item | 101 | `success: false` | `missing_docs` | 1 finding, exit 0 |
| a package whose build script breaks | 101 | `success: false` | none | 0 findings, exit 1 |
| the same package under a build script that runs | 0 | `success: true` | none | 1 finding, exit 0 |
| a workspace of two members, one whose build script breaks beside a denied lint | 101 | `success: false` | `unused_variables` | 0 findings, exit 1 |
| a tree that holds no `Cargo.toml` | 101 | none, 0 bytes | none | 0 findings, exit 1 |
| `jq` replaced by a command that exits 127 | 0 | `success: true` | none | 0 findings, exit 1 |

The earlier shape of this script wrote `set -e` and let cargo's own status be
the status of the script. Measured over each of the three deny-level rows: the
run reported 0 findings and exited 101, for a workspace clippy linted from end
to end, and every finding the report held went missing. The last row of the
three is the one the rule's own lint makes, so the rule broke the run exactly
where it had something to say.

The two-member rows are the shape `scope: workspace` makes reach a real
repository, and this repository holds more than 20 members. The member that
compiles fills the findings file, so a gate that read that file would never
reach its status test.

Eight acceptance tests hold the script to this table:
`the_shipped_rust_missing_docs_tool_rule_breaks_on_a_crate_that_does_not_compile`,
`the_shipped_rust_missing_docs_tool_rule_breaks_on_a_workspace_member_it_cannot_compile`,
`the_shipped_rust_missing_docs_tool_rule_breaks_on_a_tree_that_holds_no_manifest`,
`the_shipped_rust_missing_docs_tool_rule_breaks_on_a_build_script_that_breaks`,
`the_shipped_rust_missing_docs_tool_rule_measures_a_package_beside_a_build_script_that_runs`,
`the_shipped_rust_missing_docs_tool_rule_breaks_on_a_broken_build_script_beside_a_denied_lint`,
`the_shipped_rust_missing_docs_tool_rule_measures_a_workspace_beside_a_deny_level_lint`
and
`the_shipped_rust_missing_docs_tool_rule_breaks_when_the_filter_cannot_read_the_report`.

The `trap` removes the temporary directory when the script exits. It covers a
clean run, a run with findings and a broken run alike, and it leaves the exit
status of the script alone.

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

The lint also asks a compiled target for its own crate documentation. Measured
on a probe crate whose `src/lib.rs` and whose `build.rs` each hold no `//!`
comment: the run reports 2 findings, `missing documentation for the crate` at
line 1 of each file. A build script is a file the author writes, so that finding
stands.

## How to exempt one item

The `jq` filter selects the `missing_docs` diagnostics and drops every other
lint clippy emits. Selection here is attribution, not exemption: to exempt one
item, write `#[allow(missing_docs)]` on it in the code. Measured: the
annotation on the undocumented getter above leaves the setter as the only
finding of the run.

## The rule declares no install commands

Clippy ships as a `rustup` component of the Rust toolchain, and no package holds
it. An install command pins a package version, so no install command can pin
clippy. Clippy has a version of its own: `cargo clippy --version` reports
`clippy 0.1.97 (8bab26f4f6 2026-07-14)`, and the `check_version_command` in the
frontmatter reads it. The `doctor.fix_hint` states
`rustup component add clippy` instead, which installs it for the toolchain the
project already uses. `sah doctor` shows that hint as the fix; the install
lifecycle never runs it.
