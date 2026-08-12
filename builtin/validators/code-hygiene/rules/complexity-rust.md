---
name: complexity-rust
description: Rust functions stay under the nesting, length, argument and type gates — checked by clippy, not by prompt.
match:
  files:
    - "**/*.rs"
  project_types:
    - rust
supersedes:
  - cognitive-complexity
  - function-length
tool:
  scope: workspace
  run: |
    set -e
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    printf 'excessive-nesting-threshold = 6\ntoo-many-lines-threshold = 250\ntoo-many-arguments-threshold = 7\ntype-complexity-threshold = 250\n' > "$work/clippy.toml"
    status=0
    CLIPPY_CONF_DIR="$work" cargo clippy --workspace --all-targets --message-format=json --quiet -- \
      -W clippy::excessive_nesting -W clippy::too_many_lines \
      -W clippy::too_many_arguments -W clippy::type_complexity \
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
           | select(.code.code == "clippy::excessive_nesting"
                    or .code.code == "clippy::too_many_lines"
                    or .code.code == "clippy::too_many_arguments"
                    or .code.code == "clippy::type_complexity")
           | select(.spans | length > 0)
           | {file: .spans[0].file_name, line: .spans[0].line_start, message: .message}
           | select(.file | startswith("/") | not)' "$work/clippy.json" \
      > "$work/findings.json" || filtered=$?
    if [ "$filtered" -ne 0 ]; then
      printf 'complexity-rust: jq could not read the clippy report\n' >&2
      exit 1
    fi
    if [ "$status" -ne 0 ] && [ ! -s "$work/ran.txt" ]; then
      printf 'complexity-rust: cargo could not run clippy over the workspace\n' >&2
      exit 1
    fi
    if [ -s "$work/rustc-errors.txt" ]; then
      printf 'complexity-rust: cargo clippy could not lint the workspace\n' >&2
      exit 1
    fi
    if [ -s "$work/unrun-build-scripts.txt" ]; then
      printf 'complexity-rust: a build script did not run, so clippy did not lint every crate\n' >&2
      exit 1
    fi
    if [ "$status" -ne 0 ] && [ ! -s "$work/errors.txt" ]; then
      printf 'complexity-rust: cargo stopped the build and wrote no compiler error\n' >&2
      exit 1
    fi
    sort -u "$work/findings.json"
  doctor:
    check_command: "which cargo-clippy jq sort mktemp"
    check_version_command: "cargo clippy --version"
    fix_hint: "rustup component add clippy"
---

# Complexity and Length — Rust

`cargo clippy` decides every gate in one run. Four lints carry it:

- `clippy::excessive_nesting` — a block that sits too deep.
- `clippy::too_many_lines` — a function that runs too long.
- `clippy::too_many_arguments` — a function that takes too many parameters.
- `clippy::type_complexity` — a type written out too deep to read.

One run answers two prompt rules, so this rule names both in `supersedes`.

Every measurement below was made with clippy 0.1.97 and cargo 1.97.1.

## This rule gates nesting depth, not a cognitive score

`clippy::excessive_nesting` counts **lexical nesting depth**. It is not a
cognitive complexity score, and it is not the published Sonar algorithm the
`complexity` probe computes. It reports one thing: a block a reader has to
descend too far to reach.

Nesting depth is the backbone of the Sonar cognitive metric — that metric adds a
penalty for each level of nesting — so this lint carries the supersession of
`cognitive-complexity`. It does not reproduce that rule's number. A function
that branches wide and flat passes this gate, and the reader gets no score for
it. That is the trade: one depth every reviewer gets the same, in place of a
score an agent reads off a probe.

`clippy::cognitive_complexity` is rejected. `VALIDATOR.md` records the
measurement.

## The item overhead, and how to read the depth

Clippy counts every lexical block as one level, and the item a function sits in
counts too. A free function's body is the first level. An `impl` block is a
level of its own, so an `impl` method's body is the second. An inline `mod`
adds one more.

A block reports when its level is over the threshold. So for a threshold `T`,
control-flow depth `D` trips when:

- free function: `D + 1 > T` — at `T = 6`, six nested control-flow blocks.
- `impl` method: `D + 2 > T` — at `T = 6`, five nested control-flow blocks.

Measured on a probe crate at `T = 6`, with control-flow depth 1 through 8: a
free function reports at depth 6, 7 and 8 and is silent at depth 5; an `impl`
method reports at depth 5 and above and is silent at depth 4. Read a finding
with the item in mind — a method inside an `impl` gets one less turn of the
screw than a free function.

Only the outermost offending block of a chain reports, so a deep pyramid gives
one finding, not one for each level below the gate.

The line count of the source does not move the depth. Measured over the same
six nested blocks written on one line and written over many lines: each
reported one finding.

## The thresholds and where they come from

The script writes a `clippy.toml` into a temporary directory and points
`CLIPPY_CONF_DIR` at that directory:

- `excessive-nesting-threshold = 6` — chosen by sampling this workspace. `5`
  reports 199 findings and flags readable code; `7` reports 19 and lets a
  five-deep pyramid through; `6` reports 55 findings across 41 functions in 28
  files, and every sample read as genuine.
- `too-many-lines-threshold = 250` — the `function-length` prompt gate,
  unchanged. Clippy counts the same lines the prompt rule counts: it skips
  blank lines and comment-only lines in the function body. Measured over one
  function that holds 300 blank lines, 300 comment-only lines and 3 code
  lines: no finding.
- `too-many-arguments-threshold = 7` — clippy's own default, written out so the
  rule owns its whole configuration.
- `type-complexity-threshold = 250` — clippy's own default, for the same
  reason.

These four keys are the whole configuration these four lints have. Clippy
prints its full key list for a key it does not know, and that list holds no
other key for any of the four. It holds nine keys whose name ends in
`-in-tests`, among them `allow-unwrap-in-tests` and `allow-panic-in-tests`, and
none of the nine names one of these four lints. So no key carves out a test,
and no key carves out a data line. The section "The carve-outs the two prompt
rules state" below states what answers each carve-out instead.

`CLIPPY_CONF_DIR` was measured before the rule relied on it. A configuration
directory holding a raised threshold silences the lint on a probe crate that
trips it at the gate, so clippy reads the file. A package carrying its own
`clippy.toml` still reports when `CLIPPY_CONF_DIR` names the gate, so the
variable wins and the project's own file is never read. A cached second run
re-emits the warnings, so a repeated review still reports.

`excessive-nesting-threshold` defaults to `0`, which turns the lint off, so a
run that reports at all proves the temporary file reached clippy.

The `trap` removes the temporary directory when the script exits. It covers a
clean run, a run with findings and a broken run alike, and it leaves the exit
status of the script alone.

## How the run is shaped

The scope is `workspace` because cargo lints a package, never a loose file. The
engine keeps only the findings in the changed files. The script therefore
writes no `"$@"` and no zero-argument guard, which is what that scope asks of
it.

`--all-targets` lints the library target and the test targets separately, so
one function in one file arrives twice. Measured over one function of 302
lines: the raw report held it two times, and `sort -u` collapsed them to one.
On this workspace the raw report held 129 lines for 63 distinct findings.

`too_many_lines` also fires on generated code that cargo writes under `OUT_DIR`,
which arrives as an absolute path and is not editable. The
`select(.file | startswith("/") | not)` step drops it. Four such files were
measured on this workspace.

The run uses `-W`, never `--force-warn`. Three of the four lints warn by
default, and 35 inline `#[allow]`s for these four lints exist in this
workspace. No crate-level `#![allow]` exists for any of the four, so `-W`
reaches every function that has not been exempted on purpose.

The `jq` filter selects the four lint codes and drops every other lint clippy
emits. Selection here is attribution, not exemption: to exempt one function,
write the annotation the section below states.

The rule declares no install commands. Clippy is a component of the Rust
toolchain, not a package with its own version, so no install command can pin
it. The `doctor.fix_hint` states `rustup component add clippy` instead, which
installs it for the toolchain the project already uses. `sah doctor` shows that
hint as the fix; the install lifecycle never runs it.

## A workspace the tool cannot lint

`cargo clippy` exits nonzero for four different reasons, and one status carries
all four:

- cargo could not start a run at all.
- cargo made a run, and a crate failed its type check. Clippy runs the four
  lints AFTER that type check, so it never linted that crate.
- cargo made a run, and the BUILD SCRIPT of a crate broke. cargo runs a build
  script before it compiles the crate that script serves, so clippy never
  linted that crate. This repository holds eight build scripts.
- cargo made a run, clippy linted every crate, and a lint stands at deny level.

The first three are broken runs. The fourth is a MEASURED run, and the findings
it holds must stand. `builtin/validators/README.md` states the answer for this
shape: "One status can carry both a measured run and a broken run. The status of
a failure is then the same as the status of a finding. The script must then test
the REPORT beside the status, and accept the shared status only for the report
shape a measured run writes." The three shipped swiftlint rules make the same
test.

The FILTERED findings answer none of the four. One member that compiles fills
that file while another member never reached the lints, so a workspace with a
broken member reads as a clean tree. The script therefore reads the RAW report,
which carries what the filter drops. Three entries of it answer the question.

**`{"reason":"build-finished"}`.** cargo writes this entry for every run it
made, and no entry at all for a run it could not make. Measured over the 19
shapes below: a healthy run writes `{"reason":"build-finished","success":true}`;
every nonzero shape cargo made writes
`{"reason":"build-finished","success":false}`, the three deny-level shapes
included; a directory with no `Cargo.toml`, a `Cargo.toml` that does not parse
and a target directory cargo cannot write each write 0 bytes and no entry. So
the PRESENCE of the entry states that cargo ran, and the `success` field states
nothing the exit status does not.

**A rustc error code.** A crate that fails its type check writes a
`compiler-message` at level `error` whose code is a rustc code such as `E0308`,
and a crate that fails to parse writes one with NO code. A lint at deny level
writes a `compiler-message` at level `error` whose code is the LINT name, such
as `clippy::unwrap_used` or `unused_variables`. Measured: no error-level message
of any deny-level shape carries a rustc code or an empty code, and no
error-level message of any healthy shape stands at all.

**A build script cargo compiled and never ran.** cargo writes one
`compiler-artifact` entry whose `target.kind` holds `custom-build` for every
build script it COMPILED, and one `build-script-executed` entry for every build
script it RAN. A build script that breaks leaves the first entry standing and
the second one out. Measured over one package that holds a build script which
breaks: the whole report ran 1133 bytes, and it held one `build-finished` entry
with `success: false`, one `custom-build` artifact, no `build-script-executed`
entry and NO `compiler-message` at all. The control, the same package under
`fn main() {}`, ran 7489 bytes and held one `build-script-executed` entry beside
its artifact. So
the two entries state which build scripts ran, and cargo writes no compiler
error for this failure at all.

The pair reads a cached run the same way. Measured over one package that holds
a build script beside a path dependency that holds another: the first run wrote
two `custom-build` artifacts and two `build-script-executed` entries, and a
second run over a warm target directory, whose artifacts arrived `fresh: true`,
wrote the same two entries. Over this repository the run wrote 149 `custom-build`
artifacts and 149 `build-script-executed` entries, so no build script stands
unrun.

The script breaks the run in four places, and writes its own line for each one:

- the status is nonzero and the raw report holds no `build-finished` entry —
  `complexity-rust: cargo could not run clippy over the workspace`;
- the raw report holds an error-level message with a rustc code or with no code
  — `complexity-rust: cargo clippy could not lint the workspace`;
- the raw report holds a `custom-build` artifact whose package writes no
  `build-script-executed` entry —
  `complexity-rust: a build script did not run, so clippy did not lint every crate`;
- the status is nonzero and the raw report holds no error-level message at all
  — `complexity-rust: cargo stopped the build and wrote no compiler error`.

The last two both answer a build script that breaks, and the build-script test
stands first because it names the cause. The two are not one test. Measured
with the build-script test taken out of the script: the fourth test broke the
one-package shape and the two-member shape, and the two-member shape BESIDE a
lint at deny level still wrote `good/src/lib.rs:2` and exited 0, because the
denied lint fills the report with an error-level message. Measured with the
fourth test taken out: the build-script test broke all three. So each test
answers a shape the other one lets through.

Every other run writes the findings it holds and exits 0. cargo's own stderr
reaches the diagnosing agent beside the rule's line.

Every shape below was measured with clippy 0.1.97 and cargo 1.97.1. Each package
that holds a finding holds one function whose body runs 300 lines, which clippy
counts as 300 against the gate of 250.

| the shape | status | `build-finished` | error codes | the run answers |
|---|---|---|---|---|
| a healthy package, one finding | 0 | `success: true` | none | 1 finding, exit 0 |
| a healthy package, no finding | 0 | `success: true` | none | 0 findings, exit 0 |
| `#![deny(clippy::unwrap_used)]` beside one `unwrap`, one finding | 101 | `success: false` | `clippy::unwrap_used` | 1 finding, exit 0 |
| `[lints.clippy] unwrap_used = "deny"` beside one `unwrap`, one finding | 101 | `success: false` | `clippy::unwrap_used` | 1 finding, exit 0 |
| `#![deny(clippy::unwrap_used)]` beside one `unwrap`, no finding | 101 | `success: false` | `clippy::unwrap_used` | 0 findings, exit 0 |
| `[lints.clippy] unwrap_used = "deny"` beside one `unwrap`, no finding | 101 | `success: false` | `clippy::unwrap_used` | 0 findings, exit 0 |
| `RUSTFLAGS="-D warnings"` beside one `unused_variables`, one finding | 101 | `success: false` | `unused_variables` | 1 finding, exit 0 |
| `RUSTFLAGS="-D warnings"` beside one `unused_variables`, no finding | 101 | `success: false` | `unused_variables` | 0 findings, exit 0 |
| a package that does not compile: `pub fn broken() -> i32 { "not an integer" }` | 101 | `success: false` | `E0308` | 0 findings, exit 1 |
| a package that does not parse: `pub fn broken( {` | 101 | `success: false` | no code | 0 findings, exit 1 |
| a workspace of two members, one that does not compile and one that gives a finding | 101 | `success: false` | `E0308` | 0 findings, exit 1 |
| a directory that holds no `Cargo.toml` | 101 | none, 0 bytes | none | 0 findings, exit 1 |
| a `Cargo.toml` that does not parse | 101 | none, 0 bytes | none | 0 findings, exit 1 |
| a target directory cargo cannot write | 101 | none, 0 bytes | none | 0 findings, exit 1 |
| `jq` replaced by a command that exits 127 | 0 | `success: true` | none | 0 findings, exit 1 |
| a package whose build script breaks | 101 | `success: false` | none | 0 findings, exit 1 |
| a package whose build script runs, one finding | 0 | `success: true` | none | 1 finding, exit 0 |
| a workspace of two members, one whose build script breaks and one that gives a finding | 101 | `success: false` | none | 0 findings, exit 1 |
| the same workspace beside `[workspace.lints.clippy] unwrap_used = "deny"` | 101 | `success: false` | `clippy::unwrap_used` | 0 findings, exit 1 |

cargo writes `error: could not compile` to stderr for every nonzero row it
reached, the deny-level rows included, and `error: failed to run custom build
command` for the build-script rows, so stderr tells a reader nothing the raw
report does not. Under `RUSTFLAGS="-D warnings"` the four lints arrive at level
`error` rather than `warning`; the filter selects on the lint CODE, so the
findings stand either way.

The two-member row is the shape the `scope: workspace` of this rule makes reach
a real repository, and this repository holds more than 20 members. Measured over
that row with the earlier gate, which read the FILTERED findings file: the
member `good` wrote `good/src/lib.rs:1`, that file was not empty, the status
test never ran, and the long function of `bad/src/lib.rs` was read as clean.

The same earlier gate broke the two clean deny-level rows: a workspace clippy
linted from end to end, holding a lint at deny level and no finding of the four
gates, wrote the broken-run line and exited 1.

The three build-script rows that break are the shape the eight build scripts of
this repository make reach it. Measured over each of them with the earlier gate,
which read the compiler messages alone: the one-package row wrote 0 findings and
exited 0 while its control wrote `src/lib.rs:1`; the two-member row wrote
`good/src/lib.rs:1` alone; and the two-member row beside the denied lint wrote
`good/src/lib.rs:2` alone. In each of the three the long function of the crate
that never reached the lints was read as clean.

Nine acceptance tests hold the script to this table:
`the_shipped_rust_complexity_tool_rule_breaks_on_a_workspace_it_cannot_compile`,
`the_shipped_rust_complexity_tool_rule_breaks_on_a_workspace_member_it_cannot_compile`,
`the_shipped_rust_complexity_tool_rule_breaks_on_a_build_script_that_breaks`,
`the_shipped_rust_complexity_tool_rule_measures_a_package_beside_a_build_script_that_runs`,
`the_shipped_rust_complexity_tool_rule_breaks_on_a_member_build_script_that_breaks`,
`the_shipped_rust_complexity_tool_rule_breaks_on_a_broken_build_script_beside_a_denied_lint`,
`the_shipped_rust_complexity_tool_rule_measures_a_workspace_beside_a_deny_level_lint`,
`the_shipped_rust_complexity_tool_rule_measures_a_clean_workspace_beside_a_deny_level_lint`
and
`the_shipped_rust_complexity_tool_rule_measures_a_workspace_beside_deny_level_flags`.

## The status of each step

An earlier shape of this script was one pipe that ended in `sort -u`. A shell
pipeline takes the status of its last command, and the script writes `set -e`
with no `pipefail`, so that shape answered exit 0 for every broken run.
Measured over the package that does not compile: the pipe wrote 0 findings and
exited 0, and the engine read the whole tree as clean, the function of 302
lines included.

Each step therefore writes to a file and tests its own status. The script calls
`jq` five times — one call for the `build-finished` entry, one for the rustc
error codes, one for the error-level messages, one for the build scripts that
never ran, and one for the findings — and each call carries the same test: a
status other than 0 writes `complexity-rust: jq could not read the clippy
report` to stderr and exits 1.
Measured over the healthy package, which gives one finding, with `jq` replaced
by a command that exits 127: the pipe shape wrote 0 findings and exited 0; the
shipped shape writes 0 findings, that line, and exit 1. The acceptance test
`the_shipped_rust_complexity_tool_rule_breaks_when_the_filter_cannot_read_the_report`
holds the script to that answer.

`sort -u` reads the file the filter step wrote, so it stands last with no pipe
above it and the script takes its status.

## The annotation an author writes

Write `#[expect(<lint>, reason = "...")]` on the item a gate reports, and name
the lint the finding names:

    /// Every default this crate ships.
    #[expect(clippy::too_many_lines, reason = "one line for each field")]
    fn default() -> Self {

`#[expect]` is the marker, not `#[allow]`, because `#[expect]` expires by
itself. Measured over one function that trips `excessive_nesting` and one that
does not: the annotation on the first is silent, and the annotation on the
second raises `unfulfilled_lint_expectations`. So the compiler asks for the
annotation back the moment the function drops under the gate. `#[allow]` never
does.

Measured over one function of 302 lines against the gate of 250, and over one
function whose innermost block sits at level 7 against the gate of 6. Each of
these spellings gives no finding:

- `#[allow(clippy::too_many_lines)]` on the function.
- `#[allow(clippy::too_many_lines, reason = "a flat field list")]` on the
  function.
- `#[expect(clippy::too_many_lines, reason = "a flat field list")]` on the
  function.
- `#[allow(clippy::too_many_lines)]` on the `impl` block that holds the method.
- `#[allow(clippy::too_many_lines)]` on the `mod` that holds the function,
  inline or as an out-of-line declaration.
- `#![allow(clippy::too_many_lines)]` as the FIRST line of the function body.
- `#[allow(clippy::excessive_nesting)]` on the function, and the same
  annotation on the one statement the finding stands on.
- `#[allow(clippy::too_many_arguments)]` on the function, and
  `#[allow(clippy::type_complexity)]` on the function.

Each of these spellings gives one finding:

- `// clippy: allow too_many_lines because it is data`, or any other comment.
  Clippy reads no comment directive.
- `#[allow(clippy::too_many_lines)]` on the `macro_rules!` item that expands
  the function, and the same annotation on the macro invocation. The second one
  also raises `unused_attributes`.

`#[allow(too_many_lines)]`, with no `clippy::` in front of it, silences the
finding and raises `renamed_and_removed_lints`, which states the bare name may
stop working. Write the `clippy::` name.

`#![allow(clippy::too_many_lines)]` below the first line of a function body is
a compile error: `an inner attribute is not permitted in this context`.

The first fix a finding asks for is still to split the function, or to flatten
the block. The annotation is the second fix, and its `reason` states why.

## The carve-outs the two prompt rules state

`cognitive-complexity` exempts a test, generated code and a macro expansion,
and a long flat list of simple cases. `function-length` exempts a test,
generated code, a function that is mostly configuration or data, and an
initialization function that sets many fields.

The run reproduces the flat list for the nesting gate, and it reproduces the
generated code cargo writes under `OUT_DIR`. The author answers every other
one with the annotation above.

### A flat list of simple cases, which the nesting gate drops

`excessive_nesting` reads depth alone, so a long flat list never reaches it.
Measured over one function that holds a 301-branch `if` / `else if` chain at
depth 1: `excessive_nesting` reported nothing.

The LENGTH gate still counts that chain. The same function reported
`too_many_lines` at 302 lines. The next section states who answers that.

### Configuration, data, a builder and an initializer, which the run does not drop

`function-length` exempts "Functions that are mostly configuration/data (e.g.,
builder patterns with many options)" and "Initialization functions that set
many fields". Clippy counts a data line like a code line, and its
configuration holds no key that tells the two apart.

Measured over three functions of 302 lines each, at the gate of 250:

| the shape | `too_many_lines` |
|---|---|
| a `Config::default()` that sets 300 fields | 302/250 |
| a builder chain of 300 `.opt(n)` calls, one for each line | 302/250 |
| a `vec![]` of 300 entries | 302/250 |

So a data function REPORTS, and the author answers it. The first answer is to
move the data out of the function — a `const` table, a `Default` derive, a
smaller builder. The second answer is
`#[expect(clippy::too_many_lines, reason = "...")]` on the function, with the
reason naming the data. The acceptance test
`the_shipped_rust_complexity_tool_rule_answers_the_length_gate_annotation`
holds one annotated data function beside one bare one, and holds the run to
reporting the bare one alone.

### A test, which the run does not drop

Both prompt rules exempt a test, and `cognitive-complexity` names the
DEFINITION as the mark: "Identify a test from its attribute or framework naming
convention at the **definition**, never from the file name. A complex helper
named `build_request` in a file called `foo_test.rs` is still a complex
function and is still listed."

Clippy holds no flag and no configuration key that reads `#[test]`. Its whole
key list carries nine `-in-tests` keys, and none of the nine names one of these
four lints.

`--all-targets` is what puts test code in front of the gates, and it is the
flag, not a carve-out, that decides. Measured over one package holding a long
function in the library, a long `#[test]` function inside `#[cfg(test)] mod
tests`, a long function in `tests/it.rs` and a long function in
`examples/ex.rs`:

| the run | the files reported |
|---|---|
| with `--all-targets` | `src/lib.rs` two times, `tests/it.rs`, `examples/ex.rs` |
| without `--all-targets` | `src/lib.rs` one time, the library function alone |

The rule keeps `--all-targets`. Dropping it reads the TARGET, which is the mark
the prompt rule forbids: it silences a long helper in `tests/it.rs` beside the
test that calls it, it silences an example and a benchmark, which are ordinary
code, and it drops every `#[cfg(test)]` module because `cfg(test)` is then off.
That trades true findings for the carve-out, which is the trade
`complexity-python` refuses for a test path.

So a long test function REPORTS, and the author answers it. One
`#[expect(clippy::too_many_lines, reason = "...")]` on `mod tests` covers every
function of the module — measured, an annotation on a `mod` silences the
functions inside it. The acceptance test
`the_shipped_rust_complexity_tool_rule_reports_a_test_function_and_its_helper`
holds the run to reporting both the test and the helper beside it.

### Generated code, and a macro expansion

Rust states no generated-file header convention, and clippy reads no header.
Measured over one file whose head carries `// This file is @generated by
prost-build.` and `// Code generated by tool. DO NOT EDIT.`: it reported its
function, the same as the plain file beside it. A header test in the script
would name the first lines of one generator and never a convention, so this
rule states none. The sibling `complexity-go` does make such a test, because Go
states one convention.

Generated Rust arrives in three shapes, and each has its own answer.

- **Written under `OUT_DIR` and pulled in with `include!`.** The path is
  absolute and the file is not editable. The
  `select(.file | startswith("/") | not)` step drops it. Measured over one
  probe crate whose `build.rs` writes a function of 302 lines: the report named
  the file under `target/debug/build/.../out/out.rs` by an absolute path.
- **Checked in and named by a `mod` declaration.** The path is relative and the
  file reports. The author writes
  `#[expect(clippy::too_many_lines, reason = "...")]` on the `mod` declaration,
  which stands in the PARENT file and which the generator never writes again.
  Measured: the annotation on `pub mod generated;` silenced every finding of
  `src/generated.rs`, and the same annotation over a module file that no longer
  trips raised `unfulfilled_lint_expectations`. The acceptance test
  `the_shipped_rust_complexity_tool_rule_reports_a_generated_file` holds one
  annotated module declaration beside one bare one, over two module files that
  hold the same bytes.
- **Expanded from a macro.** `excessive_nesting` never reports one. Measured
  over the same six nested blocks written as a plain function and expanded from
  a `macro_rules!` body: the plain function reported and the expansion did not.
  `too_many_lines` does report one, at the line of the macro DEFINITION rather
  than at the invocation. The one annotation that silences it is the one the
  macro body expands: measured, `#[expect(clippy::too_many_lines, reason =
  "...")]` written above `pub fn $name()` inside the macro is silent, the same
  annotation above the `macro_rules!` item leaves the finding standing, and the
  same annotation above the invocation leaves it standing and raises
  `unused_attributes`.
