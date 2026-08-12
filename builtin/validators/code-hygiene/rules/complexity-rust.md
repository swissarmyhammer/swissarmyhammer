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
    if [ "$status" -ne 0 ] && [ ! -s "$work/findings.json" ]; then
      printf 'complexity-rust: cargo clippy could not lint the workspace\n' >&2
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

`cargo clippy` exits nonzero for two different reasons, and one status carries
both. The tool could not lint the workspace, and the tool linted the workspace
correctly while a lint stands at deny level. So the script tests the REPORT
beside the status, which is what `builtin/validators/README.md` asks of a
script whose tool shares one status between a measured run and a broken one.
The three shipped swiftlint rules make the same test.

The report of this rule is the list of findings the filter step writes. A
nonzero status beside a report that holds one finding or more is a MEASURED
run, and the script writes those findings and exits 0. A nonzero status beside
an empty report is a BROKEN run, and the script writes
`complexity-rust: cargo clippy could not lint the workspace` to stderr and
exits 1, so the engine reads a broken run rather than a clean tree. cargo's own
stderr reaches the diagnosing agent beside that line.

Every shape below was measured with clippy 0.1.97 and cargo 1.97.1, over one
package that holds a function of 302 lines against the gate of 250.

| the shape | status | the report | the run answers |
|---|---|---|---|
| the package alone | 0 | 1 finding | 1 finding, exit 0 |
| `#![deny(clippy::unwrap_used)]` beside one `unwrap` | 101 | 1 finding | 1 finding, exit 0 |
| `[lints.clippy] unwrap_used = "deny"` beside one `unwrap` | 101 | 1 finding | 1 finding, exit 0 |
| `RUSTFLAGS="-D warnings"` beside one `unused_variables` | 101 | 1 finding | 1 finding, exit 0 |
| `pub fn broken() -> i32 { "not an integer" }` beside it | 101 | empty | 0 findings, exit 1 |
| a directory that holds no `Cargo.toml` | 101 | empty | 0 findings, exit 1 |
| a `Cargo.toml` that does not parse | 101 | empty | 0 findings, exit 1 |
| a target directory cargo cannot write | 101 | empty | 0 findings, exit 1 |

cargo writes `error: could not compile` to stderr for each of the first four
nonzero rows alike, so stderr tells a reader nothing the report does not.
Under `RUSTFLAGS="-D warnings"` the four lints arrive at level `error` rather
than `warning`; the filter selects on the lint CODE, so the findings stand
either way.

A workspace that fails to compile writes its own errors into the report as
`compiler-message` entries, so the report is not empty of MESSAGES. It is empty
of the four lints this rule owns, because clippy runs those lints after the
type check the workspace failed. The acceptance test
`the_shipped_rust_complexity_tool_rule_breaks_on_a_workspace_it_cannot_compile`
holds the script to exit 1 there, and
`the_shipped_rust_complexity_tool_rule_measures_a_workspace_beside_a_deny_level_lint`
holds it to keeping the finding beside a deny-level lint.

A nonzero status beside a report of no finding is answered as a broken run even
when the workspace compiled. A workspace that fails its build at a deny-level
lint and holds no finding of these four gates therefore reports a tool error,
and the author reads cargo's own stderr beside the rule's line. That is the
same trade the three swiftlint rules make: the shared status is accepted only
for the report shape a measured run writes.

## The status of each step

An earlier shape of this script was one pipe that ended in `sort -u`. A shell
pipeline takes the status of its last command, and the script writes `set -e`
with no `pipefail`, so that shape answered exit 0 for every broken run.
Measured over the package that does not compile: the pipe wrote 0 findings and
exited 0, and the engine read the whole tree as clean, the function of 302
lines included.

Each step therefore writes to a file and tests its own status. The filter step
carries the same test as the cargo step: a status other than 0 writes
`complexity-rust: jq could not read the clippy report` to stderr and exits 1.
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
