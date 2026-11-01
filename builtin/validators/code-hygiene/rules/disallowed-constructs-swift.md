---
name: disallowed-constructs-swift
description: Swift constructs the project does not write, each with a named replacement — checked by swiftlint, not by prompt.
match:
  files:
    - "**/*.swift"
  project_types:
    - swift
tool:
  scope: files
  run: |
    set -e
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    for file in "$@"; do
      if [ ! -e "$file" ]; then
        printf 'sah-diagnostic: disallowed-constructs-swift found no file at %s, so its constructs are unread\n' "$file" >&2
      fi
    done
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    printf '%s\n' 'only_rules:' \
      '  - implicitly_unwrapped_optional' '  - force_unwrapping' '  - force_try' \
      '  - force_cast' '  - unused_optional_binding' '  - unowned_variable_capture' \
      '  - legacy_constructor' '  - legacy_nsgeometry_functions' \
      '  - legacy_cggeometry_functions' '  - custom_rules' \
      'implicitly_unwrapped_optional:' '  severity: warning' '  mode: all_except_iboutlets' \
      'force_unwrapping:' '  severity: warning' \
      '  ignored_literal_argument_functions: ["Data(hexString:)", "NSImage(named:)", "NSURL(string:)", "UIImage(named:)", "URL(string:)"]' \
      'force_try:' '  severity: warning' \
      'force_cast:' '  severity: warning' \
      'unused_optional_binding:' '  severity: warning' '  ignore_optional_try: false' \
      'unowned_variable_capture:' '  severity: warning' \
      'legacy_constructor:' '  severity: warning' \
      'legacy_nsgeometry_functions:' '  severity: warning' \
      'legacy_cggeometry_functions:' '  severity: warning' \
      'custom_rules:' \
      '  no_direct_standard_out_logs:' \
      '    name: "Writing log messages directly to standard out is disallowed"' \
      '    regex: "(\\bprint|\\bdebugPrint|\\bdump|Swift\\.print|Swift\\.debugPrint|Swift\\.dump|_printChanges)\\s*\\("' \
      '    match_kinds: [identifier]' \
      '    message: "Do not commit print(…), debugPrint(…), dump(…) or _printChanges(), which write to standard out in release. Log to a dedicated logging system, or silence one debug-only line with // swiftlint:disable:next no_direct_standard_out_logs and the reason after it"' \
      '    severity: warning' \
      '  no_file_literal:' \
      '    name: "#file is disallowed"' \
      '    regex: "(#file\\b)"' \
      '    match_kinds: [keyword]' \
      '    message: "Instead of #file, write #fileID, or #filePath where the exact path is needed"' \
      '    severity: warning' \
      '  no_unchecked_sendable:' \
      '    name: "@unchecked Sendable is discouraged"' \
      '    regex: "@unchecked Sendable"' \
      '    match_kinds: [attribute.builtin, typeidentifier]' \
      '    message: "Instead of @unchecked Sendable, write a plain Sendable conformance or a @preconcurrency import. If the type really must be @unchecked Sendable, write // swiftlint:disable:next no_unchecked_sendable above it with the synchronization invariant that makes the type thread-safe"' \
      '    severity: warning' \
      > "$work/swiftlint.yml"
    printf '%s\n' force_unwrapping force_try force_cast > "$work/product-only"
    test_globs=(Tests '*/Tests')
    if [ -f Package.swift ] &&
      swift package --scratch-path "$work/scratch" describe --type json > "$work/manifest.json" 2>/dev/null
    then
      while IFS= read -r target; do
        if [ -n "$target" ]; then
          test_globs+=("$target")
        fi
      done < <(jq -r '.targets[]? | select(.type == "test") | .path' "$work/manifest.json")
    fi
    : > "$work/test-files"
    for file in "$@"; do
      if [ ! -e "$file" ]; then
        continue
      fi
      for glob in "${test_globs[@]}"; do
        case "$file" in
          $glob/*)
            printf '%s/%s\n' "$(cd "$(dirname "$file")" && pwd -P)" "$(basename "$file")" \
              >> "$work/test-files"
            break
            ;;
        esac
      done
    done
    status=0
    lint() {
      parent="$1"
      shift
      status=0
      if [ -n "$parent" ]; then
        swiftlint lint --config "$parent" --config "$work/swiftlint.yml" \
          --force-exclude --no-cache --quiet --reporter json "$@" \
          > "$work/report.json" 2> "$work/lint.err" || status=$?
      else
        swiftlint lint --config "$work/swiftlint.yml" \
          --force-exclude --no-cache --quiet --reporter json "$@" \
          > "$work/report.json" 2> "$work/lint.err" || status=$?
      fi
    }
    project=""
    if [ -f .swiftlint.yml ]; then
      project=".swiftlint.yml"
    fi
    lint "$project" "$@"
    if [ -n "$project" ] && grep -qE '^Could not read configuration:' "$work/lint.err"; then
      printf '%s\n' 'sah-diagnostic: disallowed-constructs-swift: swiftlint cannot read .swiftlint.yml beside this rule. The run drops the project exclude list.' >&2
      lint "" "$@"
    fi
    cat "$work/lint.err" >&2
    sed -n 's/^Could not read contents of `\(.*\)`$/sah-diagnostic: swiftlint could not read the contents of \1, so its constructs are unread/p' "$work/lint.err" >&2
    measured=0
    if [ "$status" -eq 0 ]; then
      measured=1
    elif [ "$status" -eq 2 ] &&
      jq -e 'type == "array" and length > 0' "$work/report.json" >/dev/null 2>&1
    then
      measured=1
    fi
    if [ "$measured" -eq 0 ]; then
      if grep -qE '^Error: No lintable files found at paths:' "$work/lint.err"; then
        for file in "$@"; do
          if [ -e "$file" ]; then
            printf 'sah-diagnostic: disallowed-constructs-swift judged no file at %s, so its constructs are unread\n' "$file" >&2
          fi
        done
        exit 0
      fi
      exit 1
    fi
    jq -c --rawfile product_only "$work/product-only" --rawfile test_files "$work/test-files" '
      ($product_only | split("\n") | map(select(length > 0))) as $product_only
      | ($test_files | split("\n") | map(select(length > 0))) as $test_files
      | .[]
      | select(.rule_id != "warning_threshold")
      | select((.rule_id | IN($product_only[]) | not) or (.file | IN($test_files[]) | not))
      | {file: .file, line: .line, message: "\(.rule_id): \(.reason)"}
    ' "$work/report.json"
  doctor:
    check_command: "which swiftlint jq grep sed cat mktemp"
    check_version_command: "swiftlint version"
    fix_hint: "brew install swiftlint"
---

# Disallowed Constructs — Swift

`swiftlint` names a Swift construct by its shape, and every construct this gate
enables has a named replacement. A force unwrap has `??`, a `try!` has `try?`,
an `as!` has `as?`, a `CGPointMake` has `CGPoint(x:y:)`, a `print` has a logger.
So the tool's answer is a fact about the source rather than a preference: the
construct is there, the replacement exists, and the tool names which one the
project writes.

Twelve rules stand in the run. Nine are swiftlint's own, and three are custom
regex rules that this gate states in its own swiftlint configuration. The three
turn a question a reader would judge into a regex plus an escape hatch, which is
what makes them worth stating.

Every measurement below was made with swiftlint 0.65.0.

## Which rules the gate enables

Five of the twelve decide a bullet a prompt rule of `builtin/validators/swift/`
used to state. Each bullet was read out of the rule file rather than taken from
a list, and each is now out of the prompt text with this gate as its ONE owner:

| the rule | the bullet it took |
|---|---|
| `force_unwrapping` | `optionals.md` **No force unwrap (`!`) in non-test code.** |
| `implicitly_unwrapped_optional` | `optionals.md` **No implicitly unwrapped optionals (`Type!`).** |
| `force_try` | `error-handling.md` **No `try!` in non-test code.** |
| `force_cast` | `error-handling.md` **No `as!` force-cast in non-test code.** |
| `no_unchecked_sendable` | `concurrency.md` **`@unchecked Sendable` requires a documented synchronization invariant.** |

`the_shipped_swift_disallowed_constructs_tool_rule_owns_each_bullet_it_took`
holds every row to both halves of its own claim: the rule reports a file
holding that ONE construct, and the prompt rule states the bullet no longer. A
rule that went silent, and a bullet written back into the prompt text, each
fail it by name.

The other seven decide a question no shipped prompt rule asks:

| the rule | what it names |
|---|---|
| `unused_optional_binding` | an `if let _ = value` binding, which tests for `nil` the long way round |
| `unowned_variable_capture` | an `[unowned x]` capture, which crashes where a `[weak x]` capture returns `nil` |
| `legacy_constructor` | `CGPointMake` and its siblings, which the Swift initializer replaced |
| `legacy_nsgeometry_functions` | `NSWidth(rect)` and its siblings, which the struct property replaced |
| `legacy_cggeometry_functions` | `CGRectGetWidth(rect)` and its siblings, the same |
| `no_direct_standard_out_logs` | a `print`, `debugPrint`, `dump` or `_printChanges` call, which writes to standard out in release |
| `no_file_literal` | the `#file` literal, which puts the build machine's path in the binary |

## Every rule name was checked against the installed swiftlint

A name swiftlint does not know is a real hazard for a rule of this shape. So
each of the nine stock names was read out of `swiftlint rules` before it was
enabled, and swiftlint 0.65.0 knows all nine. `legacy_constant` and
`fatal_error_message` are absent from this roster, because the card that states
this gate names neither.

swiftlint answers a name it does not know with a warning and exit 0, and that is
exactly what makes the check necessary. Measured over one file holding a force
unwrap, beside a configuration naming `no_such_rule_at_all`:

| where the unknown name stands | status | findings | stderr |
|---|---|---|---|
| in `only_rules` | 0 | 1 | `warning: 'no_such_rule_at_all' is not a valid rule identifier`, then 255 lines naming every valid one |
| in an option block of its own | 0 | 1 | `warning: The key(s) 'no_such_rule_at_all' used as rule identifier(s) is/are invalid.` |

Both rows exit 0 and keep the finding of the rule swiftlint DOES know, so a
misspelled name costs one rule and reads as a healthy run. The fixture pair is
the standing guard: the failing fixture holds one declaration for each of the
twelve, so a name this rule got wrong, and a release that drops or renames a
rule, each make the pair fail and the doctor mark the rule unusable.

## The three custom rules, and what `match_kinds` buys

A regex over source text reads a comment and a string literal as readily as it
reads code. `match_kinds` is swiftlint's own answer: the match must stand in a
token of one of the named kinds. Measured over one probe file holding the four
call forms, one `print(` inside a string literal, one inside a `//` comment and
one inside a `///` doc comment:

| the run | what it reported |
|---|---|
| the shipped `match_kinds: [identifier]` | the four calls |
| the same regex with no `match_kinds` | the four calls, the string literal, the comment and the doc comment |

Three false findings is what the key removes, and no reading of the regex could
remove them.

The word boundary carries its own share. Measured on the same probe:
`sprintDistance()` reports nothing, because `\bprint` needs a boundary before
`print` and `sprint` has none. And `#fileID` reports nothing, because `(#file\b)`
needs a boundary after `#file` and `ID` is not one — which is what makes
`#fileID` the replacement the message names.

The regex reads an identifier and not a call, so a declaration named for one of
the forms reports as well. Measured: `func _printChanges() {}` reports its own
declaration line. The directive at the end of this file is the recourse, and the
same directive is the recourse the rule's own message names.

## The annotation IS the documentation

`concurrency.md` stated the `@unchecked Sendable` requirement as pure judgment:
the smell is the *absence* of a lock or an isolation mechanism and a comment.
No tool can answer "is there a documented invariant". A tool can require an
explicit annotation, and the text of that annotation IS the invariant. That
answer is the whole bullet now, and this gate is where it is written.

That is what `no_unchecked_sendable` does, and the message says so in as many
words: write `// swiftlint:disable:next no_unchecked_sendable` above the
declaration with the synchronization invariant after it. An author who reads the
finding is told exactly what to write.

Measured over two classes holding the same bytes, one under the directive and
one without it: the class without it reports, and the class under it reports
nothing. The passing fixture carries the annotated form, so a swiftlint release
that stopped honouring the directive makes the fixture pair fail.

## Test targets keep the three force rules off

The two bullets above say "in non-test code" and they mean it. A test asserts on a
value it has already proved is there, and a force unwrap is how a test says so. So
`force_unwrapping`, `force_try` and `force_cast` report in a product target and
stay silent in a test target. The other nine rules read every file.

**The split cannot live in the configuration.** swiftlint has no per-rule path
filter for a stock rule, and it does not refuse the key either. Measured over one
force unwrap under `Sources/` and the same bytes under `Tests/`, with
`force_unwrapping:` carrying `excluded: ".*/Tests/.*"`: swiftlint reported BOTH
files at exit 0 and wrote `warning: Configuration for 'force_unwrapping' rule
contains the invalid key(s) 'excluded'.` A rule that stated the key would have
looked correct and measured nothing.

So the script partitions its own paths, and it reads two facts to do it.

- **The package manifest, which is authoritative.** When the workspace root
  holds a `Package.swift`, the run asks `swift package describe --type json`
  which targets are of type `test` and takes their `path`. Measured over a probe
  package declaring `.testTarget(name: "OddTests", path: "Custom/OddTests")`: the
  manifest names `Custom/OddTests`, and the run keeps the force rules off there.
- **The `Tests/` directory, which is the convention.** A path whose first
  component is `Tests`, or which holds a `Tests` component anywhere, is test
  code. That is SPM's own default layout — `dead-code-swift` records the same
  convention over Alamofire, which declares one test target at `Tests`, and
  swift-nio, which declares fifteen at `Tests/<Name>`.

The two are a UNION rather than a choice, so an Xcode project with no
`Package.swift` still gets the convention, and a package whose test target
stands outside `Tests/` still gets the manifest's answer. Measured over the probe
package above, with `swift` taken off the PATH: `Tests/ProbeTests` keeps the
carve-out and `Custom/OddTests` loses it. That is the whole cost of a machine
with no Swift toolchain, and it costs a finding rather than losing one.

The manifest read is guarded, never required. `[ -f Package.swift ]` stands
first, the `2>/dev/null` swallows a manifest the tool cannot read, and a
nonzero status leaves the convention standing on its own — so `swift` is absent
from `doctor.check_command` and a machine without it keeps the whole gate.
`--scratch-path` sends the build directory `swift package` writes into the
temporary directory rather than into the workspace. Measured over the probe
package: the workspace holds no `.build` after the run.

**The partition matches on the resolved absolute path.** swiftlint writes the
symlink-resolved absolute path into each finding, so the script resolves each
test path the same way — `cd "$(dirname "$file")" && pwd -P` beside the base
name — and the `jq` filter compares whole strings. Measured: that expression
reproduces swiftlint's own `file` field exactly, through a `./` prefix on the
argument and through a working directory reached by a symlink. A regex over the
path was refused for the reason a regex is always refused here: a directory name
holding a regex metacharacter would silently stop matching.

The run itself is ONE swiftlint invocation over every path. Splitting the run in
two, one for each partition, would double the status test and the three stderr
readings below, and the shapes those tests answer are the same shapes for both
halves.

## Two prompt carve-outs no option expresses

Three of the five bullets above carried a sanctioned exception of their own,
and swiftlint expresses ONE of them. `swiftlint rules <name>` names the whole
option set each rule accepts, and the other two exceptions are absent from it.
This gate owns the three bullets now, so this table is where each exception is
stated.

| the bullet's exception | the option that expresses it |
|---|---|
| no IUO, except `@IBOutlet` | `mode: all_except_iboutlets`, which the child states |
| no IUO, except a test fixture set in `setUp()` | none — `mode` and `severity` are the whole option set |
| no `try!`, except a literal that can fail solely through programmer error | none — `severity` is the whole option set |

Measured over one file: `@IBOutlet var titleLabel: UILabel!` reports nothing,
and `var plain: UILabel!` beside it reports. A `var subject: Screen!` a `setUp()`
method assigns reports as well, and no option moves it — the mode reads the
attribute and never the assignment.

So an author writes the directive for those two, with the reason after it, which
is the same answer the section on the directive gives for every other line. The
gate reports the shape and the author states why this one stands; that trade is
the point of the whole set, and it is what makes the exception legible to the
next reader instead of a judgment each reviewer makes again.

## Every rule states `severity: warning`

`force_try` and `force_cast` default to `severity: error`, and `swiftlint rules`
names that default for each. An error-severity finding makes swiftlint exit 2,
and the status test below accepts status 2 only beside a report of one entry or
more — so the default would have worked, and it would have made the run's status
depend on which rule found something. The child states `severity: warning` for
all twelve instead, so a run that measured exits 0.

A child block replaces the parent block whole, which is what stops a project from
moving any of it.

## The rule owns what it measures

Measured over one file holding a force unwrap, a `print` call and an unused
optional binding, beside each project `.swiftlint.yml`:

| the project configuration | findings |
|---|---|
| none | the same 3 |
| `disabled_rules: [force_unwrapping]` | the same 3 |
| `only_rules: [line_length]` | the same 3 |
| a `custom_rules:` block redefining `no_direct_standard_out_logs` with a regex that matches nothing | the same 3 |

The project cannot turn a rule of this gate off, and it cannot disarm a custom
rule by redefining it: the child's `custom_rules:` block replaces the parent's
block whole, exactly as a stock rule's option block does.

What the project DOES own is its `excluded:` list, which is the generated-code
carve-out `function-length-swift` records, reached the same way. Measured over
two files holding the same force unwrap, one under `Generated/` and one under
`Sources/`, beside a project file stating `excluded: [Generated]`: the run
reports 1.

## How the run is shaped

The scope is `files` because swiftlint reads the paths it is given, and the
script counts its arguments first — `swiftlint lint` with no path walks the whole
tree under the working directory, and that answer exits 0, so it reads as a
measured result.

Everything from the two `--config` paths down to the status test is the shape the
four shipped swiftlint rules share, and `magic-numbers-swift` states each
measurement behind it:

- the PARENT config is the project's own `.swiftlint.yml`, named only when the
  file is there, which gives the run the project's `excluded:` list;
- the CHILD config is the file above, which the script writes into its temporary
  directory, so the rule owns what it measures;
- `--force-exclude` applies the `excluded:` list to a path named on the command
  line, and `--no-cache` keeps swiftlint from writing a cache into the workspace;
- the script writes swiftlint's report to a FILE rather than into a pipe, so
  swiftlint's own status reaches the gate;
- the status test accepts 0, and accepts 2 only beside a report holding one entry
  or more, because a version mismatch also exits 2 and writes 0 bytes;
- stderr is read three times, each reading anchored on the start of a line so it
  takes swiftlint's own message and never a file name that happens to hold the
  same words.

Measured with the shipped script, each refusing path staged beside
`Sources/Dirty.swift`, which holds one finding:

| the refusing path | findings | marked lines | exit |
|---|---|---|---|
| a path that holds no file | 1 | 1 | 0 |
| a file whose bytes are not UTF-8 | 1 | 1 | 0 |
| a file with no read permission | 1 | 1 | 0 |
| a path that holds no file, alone | 0 | 1 | 0 |
| a run with no argument at all | 0 | 0 | 0 |

Every row keeps the finding of the file the run DID judge, which is what
`builtin/validators/README.md` asks for: "a nonzero exit fails the WHOLE run, so
one unjudged path throws away every finding the run did make."

The `jq` filter drops the `warning_threshold` entry swiftlint adds at a project
threshold, and it drops a force-rule finding whose file the partition above named
as test code. Selection there is attribution, not exemption.

## Why this rule supersedes nothing

`supersedes` names a WHOLE prompt rule, and the engine skips that rule whole when
the tool is healthy. This gate decides FIVE bullets spread across three prompt
rules of the `swift` set — two of `optionals.md`, two of `error-handling.md` and
one of `concurrency.md` — and none of the three was only those bullets.
`optionals.md` still holds three bullets, `error-handling.md` three and
`concurrency.md` six, so naming any one here would take those out of every
review the moment swiftlint is installed.

The prompt half has landed. All five bullets are out of the prompt text and
this gate is their one owner. One requirement takes one owner, and
`the_shipped_swift_disallowed_constructs_tool_rule_owns_each_bullet_it_took`
is what holds it that way.

`stuttering-name-go`, `unused-dependencies-rust` and `idioms-swift` are the
shipped tool rules that already declare no `supersedes`, so an empty key is the
stated shape rather than an omission.

## The directive an author writes

Selection in the roster is attribution, not exemption. To exempt one line, write
`// swiftlint:disable:next <rule>` on the line DIRECTLY above it, and name the
rule the finding names — the message carries it, because the script writes
`<rule_id>: <reason>` as the finding text:

    // swiftlint:disable:next force_unwrapping  the caller tested for nil above
    return value!

Measured: the directive silences a stock rule and a custom rule alike. The
passing fixture carries one of each, so the escape hatch stays measured.

`// swiftlint:disable <rule>` runs to a matching `// swiftlint:enable <rule>`,
and the `excluded:` list of the project's own `.swiftlint.yml` covers a whole
directory.

The reason after the directive is not decoration. For `no_unchecked_sendable` it
is the whole point, and the section above states why.

## What the fixture pair holds

The failing fixture holds one declaration for each of the twelve rules, under a
`// MARK:` heading that names the group. Measured with the shipped script: **12
findings**, one for each enabled rule, exit 0, and 0 bytes on stderr.

The passing fixture holds the SAME declarations, each written in the form its
rule asks for, and two of them behind the inline directive instead. Measured with
the shipped script: 0 findings, exit 0, and 0 bytes on stderr.

The doctor materializes the fixture directory flat, so neither fixture stands
under a test target and the three force rules apply to both. The acceptance tests
stage a probe repository of their own for the test-target split.

## The rule declares no install commands

Homebrew is the supported way to install swiftlint and it installs the current
version only, so a Homebrew command cannot pin one — the same reason
`function-length-swift`, `magic-numbers-swift`, `missing-docs-swift` and
`dead-code-swift` each state. The `doctor.fix_hint` names the Homebrew command
instead. `sah doctor` shows that hint as the fix; the install lifecycle never
runs it.
