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
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    printf 'excessive-nesting-threshold = 6\ntoo-many-lines-threshold = 250\ntoo-many-arguments-threshold = 7\ntype-complexity-threshold = 250\n' > "$work/clippy.toml"
    CLIPPY_CONF_DIR="$work" cargo clippy --workspace --all-targets --message-format=json --quiet -- \
      -W clippy::excessive_nesting -W clippy::too_many_lines \
      -W clippy::too_many_arguments -W clippy::type_complexity |
      jq -c 'select(.reason == "compiler-message")
             | .message
             | select(.code.code == "clippy::excessive_nesting"
                      or .code.code == "clippy::too_many_lines"
                      or .code.code == "clippy::too_many_arguments"
                      or .code.code == "clippy::type_complexity")
             | select(.spans | length > 0)
             | {file: .spans[0].file_name, line: .spans[0].line_start, message: .message}
             | select(.file | startswith("/") | not)' |
      sort -u
  doctor:
    check_command: "which cargo-clippy jq mktemp"
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

## The thresholds and where they come from

The script writes a `clippy.toml` into a temporary directory and points
`CLIPPY_CONF_DIR` at that directory:

- `excessive-nesting-threshold = 6` — chosen by sampling this workspace. `5`
  reports 199 findings and flags readable code; `7` reports 19 and lets a
  five-deep pyramid through; `6` reports 55 findings across 41 functions in 28
  files, and every sample read as genuine.
- `too-many-lines-threshold = 250` — the `function-length` prompt gate,
  unchanged. Clippy counts the same lines the prompt rule counts: it skips
  blank lines and comment-only lines in the function body.
- `too-many-arguments-threshold = 7` — clippy's own default, written out so the
  rule owns its whole configuration.
- `type-complexity-threshold = 250` — clippy's own default, for the same
  reason.

`CLIPPY_CONF_DIR` was measured before the rule relied on it. A configuration
directory holding a raised threshold silences the lint on a probe crate that
trips it at the gate, so clippy reads the file. A package carrying its own
`clippy.toml` still reports when `CLIPPY_CONF_DIR` names the gate, so the
variable wins and the project's own file is never read. A cached second run
re-emits the warnings, so a repeated review still reports.

`excessive-nesting-threshold` defaults to `0`, which turns the lint off, so a
run that reports at all proves the temporary file reached clippy.

The `trap` removes the temporary directory when the script exits, and it leaves
the pipe's exit status as the script's exit status.

## How the run is shaped

The scope is `workspace` because cargo lints a package, never a loose file. The
engine keeps only the findings in the changed files.

`--all-targets` lints the library target and the test targets separately, so
one function in one file arrives twice. The raw pipe emitted 129 lines for 63
distinct findings on this workspace. `sort -u` collapses them.

`too_many_lines` also fires on generated code that cargo writes under `OUT_DIR`,
which arrives as an absolute path and is not editable. The
`select(.file | startswith("/") | not)` step drops it. Four such files were
measured on this workspace.

The run uses `-W`, never `--force-warn`. Three of the four lints warn by
default, and 36 legitimate inline `#[allow]`s exist in this workspace. No
crate-level `#![allow]` exists for any of the four, so `-W` reaches every
function that has not been exempted on purpose.

The `jq` filter selects the four lint codes and drops every other lint clippy
emits. Selection here is attribution, not exemption: to exempt one function,
write `#[allow(clippy::excessive_nesting)]` — or the matching lint name — on it
in the code.

The rule declares no install commands. Clippy is a component of the Rust
toolchain, not a package with its own version, so no install command can pin
it. The `doctor.fix_hint` states `rustup component add clippy` instead, which
installs it for the toolchain the project already uses. `sah doctor` shows that
hint as the fix; the install lifecycle never runs it.
