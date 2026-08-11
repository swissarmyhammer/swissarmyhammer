---
name: magic-numbers-swift
description: Unnamed Swift literals need constants — checked by swiftlint, not by prompt.
match:
  files:
    - "**/*.swift"
  project_types:
    - swift
supersedes: magic-numbers
tool:
  scope: files
  run: |
    set -e
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    for file in "$@"; do
      if [ ! -r "$file" ]; then
        printf 'magic-numbers-swift cannot read %s\n' "$file" >&2
        exit 1
      fi
    done
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    printf '%s\n' 'only_rules:' '  - no_magic_numbers' \
      'no_magic_numbers:' '  allowed_numbers: [0, 1, -1, 100]' > "$work/swiftlint.yml"
    lint() {
      if [ -f .swiftlint.yml ]; then
        swiftlint lint --config .swiftlint.yml --config "$work/swiftlint.yml" \
          --force-exclude --no-cache --quiet --reporter json "$@"
      else
        swiftlint lint --config "$work/swiftlint.yml" \
          --force-exclude --no-cache --quiet --reporter json "$@"
      fi
    }
    status=0
    lint "$@" > "$work/report.json" 2> "$work/lint.err" || status=$?
    cat "$work/lint.err" >&2
    if [ "$status" -ne 0 ]; then
      if grep -qF 'No lintable files found' "$work/lint.err"; then
        exit 0
      fi
      exit 1
    fi
    jq -c '.[] | select(.rule_id == "no_magic_numbers")
           | {file: .file, line: .line, message: .reason}' "$work/report.json"
  doctor:
    check_command: "which swiftlint jq grep mktemp"
    check_version_command: "swiftlint version"
    fix_hint: "brew install swiftlint"
---

# Magic Numbers — Swift

`swiftlint` reports every unnamed numeric literal. The `no_magic_numbers` rule
names that check. It is opt-in, so it never runs unless a configuration turns it
on.

The rule is already close to the `magic-numbers` prompt carve-outs. Measured
against a probe file holding one literal of each kind, it reported nothing for a
variable declaration, a stored property, a `static let`, an enumeration raw value,
or a default parameter — each of those declarations names its value. `100` is
absent too, which is the prompt carve-out for percent.

`allowed_numbers` is the one threshold the rule sets. The swiftlint default is
`[0.0, 1.0, 100.0]`, which is the prompt carve-out list without `-1`, so the
config states `[0, 1, -1, 100]` and the two lists then agree. `magic-numbers-go`
and `magic-numbers-typescript` state the same four values in their own
allow-lists.

## The shift carve-out is expressed, and Swift alone expresses it

The prompt rule names two conventional values, and this rule restores both.
`100` for percent is a VALUE, so `allowed_numbers` states it. A `<< 8` is a
POSITION — the operand of a shift — and no value allow-list can state a
position. `magic-numbers-go` and `magic-numbers-typescript` each record that
their own list cannot: a list carrying `8` silences a genuine `status == 8`
beside `word << 8`, which trades a real finding for a carve-out.

swiftlint answers it without the list, because `no_magic_numbers` reads the
OPERATOR. Measured on swiftlint 0.65.0 against the shipped
`allowed_numbers: [0, 1, -1, 100]`: `return word << 8` and `return word >> 8`
report nothing, and `return status == 8` reports
`Magic numbers should be replaced by named constants`. Both operands are carved
out, so `return 4096 << width` is silent as well.

The carve-out is the shift operator and nothing else. Measured in one identical
shape, `return word <operator> 8`: `<<` and `>>` are silent, and `&<<` — the
masking shift — `*`, `+`, `&`, `|`, `^` and `==` each report their `8`.

So `8` stays OUT of `allowed_numbers`. The carve-out is already reached, and
adding `8` would buy nothing and lose `status == 8`.

### The carve-out reaches a whole shift, not a link of a longer chain

The carve-out holds when the shift is the WHOLE expression at its position. It
does not reach a shift that stands as one link of a longer unparenthesised
operator chain, because swiftlint then reads the chain rather than a shift.
Measured on the same probe against the same config:

| Written | Reported |
|---|---|
| `return word << 8` | no |
| `let packed = word << 8` | no |
| `schedule(value: word << 8)` | no |
| `acc = (word << 8)` | no |
| `return (word << 8) \| 1` | no |
| `if (word << 8) > 0` | no |
| `acc = word << 8` | yes |
| `return word << 8 \| 1` | yes |
| `if word << 8 > 0` | yes |
| `return flag ? word << 8 : word` | yes |

Two recourses answer the four that report, and both are measured. Parentheses
around the shift silence every one of them, and that is the clearer code
besides. The other is the inline suppression at the end of this file: write
`// swiftlint:disable:next no_magic_numbers` above the line, with the reason
after it, which silenced `return word << 8 | 1`.

No option answers the rest. `swiftlint rules no_magic_numbers` names the whole
set the rule accepts — `severity`, `test_parent_classes` and `allowed_numbers`.
None of the three names a shift, and a fourth key is refused: an added
`allowed_shifts` makes swiftlint answer `Configuration for 'no_magic_numbers'
rule contains the invalid key(s) 'allowed_shifts'.` and read it no further.

Both halves are held by fixtures. The pass fixture carries `return word << 8`
and `return word >> 8`, so a swiftlint release that dropped the carve-out makes
the fixture pair fail and the doctor mark the rule unusable. The fail fixture
carries `return word << 8 | 1`, and the acceptance test
`the_shipped_swift_magic_numbers_tool_rule_reports_every_fail_fixture_line`
holds swiftlint to reporting it, so the edge stays measured.

## How the run is shaped

The script names TWO configuration files. swiftlint reads a list of `--config`
paths as a parent-child hierarchy. The three shipped swiftlint rules share this
shape, and `missing-docs-swift` states each measurement behind it.

- The PARENT is the project's own `.swiftlint.yml` at the repository root. The
  script names it only when the file is there, because a `--config` path that
  holds no file aborts swiftlint. The parent gives the run the project's
  `excluded:` list.
- The CHILD is the file the script writes into a temporary directory. It states
  `only_rules: [no_magic_numbers]` and the rule's `allowed_numbers`, so the rule
  owns what it measures.

`--force-exclude` makes swiftlint apply the `excluded:` list to a file named as
a command-line argument. `--no-cache` keeps swiftlint from writing a cache
directory into the workspace.

The scope is `files` because swiftlint reads the paths it is given.

## The project's own `excluded:` list decides which files are read

The `magic-numbers` prompt rule names no carve-out by path, and swiftlint holds
no check of its own that reads a path: `no_magic_numbers` reads no header line
and no file name. A project states which directories the linter passes over in
the `excluded:` list of its own `.swiftlint.yml` — the directory its generator
writes into, and its vendored trees — and that list is the whole path carve-out.

Measured over two files that each hold `return status == 404`, one under
`Generated/` and one under `Sources/`, beside a `.swiftlint.yml` that states
`excluded: [Generated]`:

| the run | findings |
|---|---|
| the shipped script | 1 |
| the same script with `--force-exclude` removed | 2 |
| the same script that never names the project configuration | 2 |

The acceptance test
`the_shipped_swift_magic_numbers_tool_rule_reads_the_project_exclude_list` holds
the run to the 1 finding of the first row.

## A run whose every file the project excludes

`--force-exclude` can leave swiftlint no file to read. swiftlint then exits 1
and writes `Error: No lintable files found at paths: ...` to stderr, which reads
as a broken tool. A change that touched generated code alone would then answer
with a tool error rather than with a clean list.

The script tests each file it is given for readability before it starts, so that
message can carry one cause only: the exclude list took every file. Measured
over one file under `Generated/` beside `excluded: [Generated]`: the run reports
no finding, exits 0, and writes swiftlint's own message to stderr. The
acceptance test
`the_shipped_swift_magic_numbers_tool_rule_answers_zero_when_the_project_excludes_every_file`
holds that behaviour.

## The rule owns its own allow-list

A project configuration can state options for `no_magic_numbers`. The child's
`no_magic_numbers:` block replaces the parent's block whole, so the project
cannot change what the rule measures.

Measured against a project configuration that states `disabled_rules:
[no_magic_numbers]` and `allowed_numbers: [404]`, over one file holding
`return status == 404`: the run reports 1 finding. The same run with the
`no_magic_numbers:` block removed from the script reports 0. The acceptance test
`the_shipped_swift_magic_numbers_tool_rule_keeps_its_own_allowed_numbers` holds
that pair of counts.

## A run cannot answer zero for a broken tool

swiftlint exits 1 for a file that is not there, and it writes nothing to stdout.
A shell pipeline takes the exit status of its LAST command, and that command was
`jq`, so the earlier pipe exited 0 and reported nothing. That reads exactly like
a clean file.

The script tests each file it is given before it starts, and it writes
swiftlint's report to a file rather than into a pipe. Measured over one path that
holds no file: the earlier pipe reported no finding and exited 0; the script
reports no finding and exits 1, with `magic-numbers-swift cannot read
Sources/Absent.swift` on stderr. The acceptance test
`the_shipped_swift_magic_numbers_tool_rule_breaks_on_a_file_it_cannot_read`
holds that behaviour.

`mktemp -d` makes the working directory the script writes the configuration and
the report into, and `trap 'rm -rf "$work"' EXIT` removes it. The trap covers
every way the script leaves: a clean run, a finding, and a failure.

## A run answers for the files it is given, and for no other

`swiftlint lint` with no path argument walks the whole tree under the working
directory. A `files`-scope script that hands `"$@"` straight to swiftlint
therefore answers for every Swift file under the repository root when the run
carries no file. That answer exits 0, so it reads as a measured result.

The script counts its arguments first. A count of zero exits 0 with no finding.
Measured over a probe tree of two files that each hold `return status == 404`,
with no argument: without the guard the script reported 2 findings and exited 0;
with the guard it reports none and exits 0. The acceptance test
`the_shipped_swift_magic_numbers_tool_rule_reads_only_the_files_it_is_given`
holds that behaviour.

## The rule declares no install commands

Homebrew is the supported way to install swiftlint, and it installs the current
version only, so a Homebrew command cannot pin one. Mint can pin one —
`mint install realm/SwiftLint@0.65.0` — but it builds swiftlint from source and
links the result into `~/.mint/bin`, which is not on the path, so the command
cannot make `check_command` pass. The `doctor.fix_hint` states
`brew install swiftlint` instead. `sah doctor` shows that hint as the fix; the
install lifecycle never runs it.

## How to exempt one literal

Selection in the filter is attribution, not exemption: to exempt one literal,
write `// swiftlint:disable:next no_magic_numbers` above it in the code. To
exempt a whole directory, add it to the `excluded:` list of the project's own
`.swiftlint.yml`.
