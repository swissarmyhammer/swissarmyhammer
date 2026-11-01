---
name: missing-docs-swift
description: Public Swift declarations need docs — checked by the toolchain's own `swift format`, not by prompt.
match:
  files:
    - "**/*.swift"
  project_types:
    - swift
supersedes: missing-docs
tool:
  scope: files
  run: |
    set -e
    if [ "$#" -eq 0 ]; then
      exit 0
    fi
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    probe=sah-probe.swift
    allowed=AllPublicDeclarationsHaveDocumentation
    printf '{"rules":{"%s":true}}\n' "$allowed" > "$work/rules.json"
    for file in "$@"; do
      if [ ! -e "$file" ]; then
        printf 'sah-diagnostic: missing-docs-swift found no file at %s, so its declarations are unread\n' "$file" >&2
        continue
      fi
      if [ ! -f "$file" ] || [ ! -r "$file" ]; then
        printf 'sah-diagnostic: missing-docs-swift declined %s: the path is not a readable file\n' "$file" >&2
        continue
      fi
      status=0
      swift format lint --strict --configuration "$work/rules.json" \
        --assume-filename "$probe" - < "$file" > /dev/null 2> "$work/lint.err" || status=$?
      : > "$work/kept"
      : > "$work/refused"
      awk -v file="$file" -v probe="$probe" -v allowed="$allowed" \
        -v kept="$work/kept" -v refused="$work/refused" '
        BEGIN {
          opening = probe ":"
          size = length(opening)
        }
        {
          at = index($0, opening)
          rest = at > 0 ? substr($0, at + size) : ""
          if (at == 0 || match(rest, /^[0-9]+:[0-9]+: error: /) == 0) {
            print $0 > refused
            next
          }
          split(substr(rest, 1, RLENGTH), part, ":")
          mark = substr(rest, RLENGTH + 1)
          if (mark !~ /^\[[A-Za-z]+\] /) {
            print rest > refused
            next
          }
          shut = index(mark, "]")
          tag = substr(mark, 2, shut - 2)
          if (tag == allowed) {
            print file ":" part[1] ": " tag ": " substr(mark, shut + 2) > kept
          }
        }' "$work/lint.err"
      trouble="$(sed -n '1p' "$work/refused")"
      if [ -n "$trouble" ] || { [ "$status" -ne 0 ] && [ "$status" -ne 1 ]; }; then
        printf 'sah-diagnostic: missing-docs-swift declined %s: %s\n' \
          "$file" "${trouble:-swift format exited $status}" >&2
        continue
      fi
      cat "$work/kept"
    done
  doctor:
    check_command: "which swift mktemp awk cat rm sed && printf '' | swift format lint --strict --configuration '{}' --assume-filename sah-probe.swift -"
    check_version_command: "swift --version"
    fix_hint: "install the Swift toolchain, Swift 6.2 or newer — Xcode 26 and above ship it"
---

# Missing Documentation — Swift

The Swift toolchain reports every `public` declaration that has no doc comment.
Its own rule `AllPublicDeclarationsHaveDocumentation` names that check, and the
gate runs `swift format lint` to read it. The gate never rewrites a file.
Review is read-only, so a finding names the line and the author makes the edit.

`swift format` is a SUBCOMMAND of the Swift toolchain, spelled with a space. It
is a different program from `swiftformat`. This gate runs the toolchain, so it
needs no install step at all. The earlier form of this gate ran `swiftlint`,
which only Homebrew installs.

Every measurement below was made with the toolchain of this machine and of the
self-hosted macOS CI runner:

    Apple Swift version 6.4.2 (swiftlang-6.4.2.1.9 clang-2100.4.1.1)
    Target: arm64-apple-macosx27.2.0

`swift format --version` writes `main`, so it names no release. The Swift
version above is the only version a measurement can carry.

## Which Swift version gives the rule

The rule is old. The file `AllPublicDeclarationsHaveDocumentation.swift` of the
`swiftlang/swift-format` repository answers HTTP 200 on `release/5.6`,
`release/5.7`, `release/5.8`, `release/5.9`, `release/5.10`, `release/6.0` and
`release/6.2`. It answers HTTP 404 on `release/5.3` and `release/5.5`. So the
rule ships with Swift 5.6 and newer.

The rule does not set the floor of the GATE. The `swift format` subcommand
arrives with Swift 6.0, and the option pair `--assume-filename sah-probe.swift -`
is accepted only at Swift 6.2 and newer: `release/6.0` and `release/6.1` guard
it with `if assumeFilename != nil && !paths.isEmpty`, and `-` IS a path. The
guards were read from the source, and the behavior was measured on Swift 6.4.
No Swift 6.0 or 6.1 toolchain was available. The floor is therefore **Swift
6.2**, which Xcode 26 and above ship, and `doctor.fix_hint` says so.
`doctor.check_command` gives the tool the same three options the run block
gives it, so a toolchain that is too old reports as broken, and never as
healthy over a gate that declines every file. `idioms-swift` states the full
measurement of this floor.

The CI runner runs Swift 6.4, which stands over the floor.

## The configuration alone is not the gate

`AllPublicDeclarationsHaveDocumentation` is OFF in the default configuration of
the tool. The script writes a small JSON configuration that turns it ON, and
that `rules` table REPLACES the default table, so every other rule is OFF. Two
rules of the tool, `ValidateDocumentationComments` and
`BeginDocumentationCommentWithOneLineSummary`, decide the bullets of
`builtin/validators/swift/rules/doc-parameter-naming.md`. That is a different
requirement with a different owner, so the configuration leaves both OFF.

The configuration still is not the gate. `swift format lint` always writes the
layout findings of its pretty-printer and of its whitespace linter —
`[Indentation]`, `[Spacing]`, `[LineLength]`, `[AddLines]` and others — and no
key of the configuration turns them off. Measured over the shipped passing
fixture, with a configuration that makes ON only this one rule:

    Pass.swift:9:1: error: [Indentation] unindent by 2 spaces
    Pass.swift:10:1: error: [Indentation] unindent by 2 spaces

Both fixtures of this rule are written with 4-space indentation, which is the
house style of every Swift fixture of this set, and `swift format` asks for 2.
So the script reads the `[<RuleName>]` tag off each output line and KEEPS ONLY
`AllPublicDeclarationsHaveDocumentation`. Every other tag is dropped.

| the run over | findings |
|---|---|
| the shipped passing fixture, `swift format lint --strict` with the configuration and no filter | 2, both `[Indentation]` |
| the shipped passing fixture, the shipped script | 0, exit 0 |
| the shipped failing fixture, `swift format lint --strict` with the configuration and no filter | 4, three of the rule and one `[Indentation]` |
| the shipped failing fixture, the shipped script | **3**, exit 0 |

The failing fixture holds one documented neighbor, one undocumented structure,
one undocumented method and one undocumented function. The three findings are
the structure, the method and the function.

## How the run is shaped

The script reads the tool's output from stderr. Findings and errors share that
one channel, and stdout holds 0 bytes. `--strict` is what makes each line say
`error:`. `swift format lint --strict` exits 1 when it reports, so the script
captures the status with `|| status=$?`, and `set -e` never ends the run on a
finding.

The status cannot tell a judged file from a refused one, because a finding and
a parse error both exit 1. So the script reads the SHAPE of each line. A line
whose FIRST `sah-probe.swift:` carries `<line>:<column>: error: [<Tag>] ` right
after it is a tagged finding, and every other line is trouble. A file that
wrote a trouble line is declined whole, because a file the parser could not
read is a file no rule judged. Measured over `public func oops( {`: two untagged
`error:` lines, exit 1, so the script writes one marked line and no finding.

The script hands the tool the SOURCE on standing input under one assumed name
that holds no `:`. The tool then writes `sah-probe.swift:<line>:<column>:
error: ...` whatever the file is named, so neither the name of the file nor the
bytes in it can move the head the reading looks for. `idioms-swift` states the
measurements behind that shape. The finding the script writes carries the path
the argument list spelled, the line, the tag and the tool's sentence.

The script runs the tool ONCE for each file. A path that names a directory makes
the tool exit 64, and a shared run would lose every finding beside it. A path
that holds no file makes the tool exit 0 and write NOTHING, which reads exactly
like a clean pass over a file the run never opened. So the script tests each
path before the tool starts, and writes one line opening `sah-diagnostic:` for
a path it could not judge, at exit 0. `builtin/validators/README.md` states that
channel.

| the path | what the script does |
|---|---|
| a path that holds no file | one marked line, no tool run |
| a directory, or a file with no read permission | one marked line, no tool run |
| a file whose bytes are not UTF-8 | the tool writes an untagged line, so one marked line |
| a file the parser cannot read | the tool writes untagged lines, so one marked line |
| a file that holds findings | the findings, exit 0 |

The scope is `files`, and a run given no file exits 0 at once with no finding.

The project's own `.swift-format` does not reach the run, because the script
names `--configuration`. Measured over a project `.swift-format` that sets
`AllPublicDeclarationsHaveDocumentation` to `false`: the run still reports the
undocumented declarations. A project cannot switch the gate off.

## What the tool reports, and what it leaves alone

Measured with Apple Swift 6.4.2, over probe files that hold each shape below.

REPORTED:

- A `public` struct, class, enum, protocol, function, method, initializer,
  subscript, property, constant and typealias that has no doc comment. Both `///` and `/** */` count as doc comments. A plain
  `//` comment does not.
- A trivial `public init() {}`.
- A getter, in every type. A `public var name: String { "" }`
  inside `public struct Wide: Equatable` reports, and so does the `public var
  description` of a protocol conformance written in an extension. The type's
  inherited types change nothing.
- A `public` member of a `private` type, and a `public` helper in a test file.
  The tool reads the access modifier and the doc comment, and never the path.

NOT REPORTED:

- A declaration with no `public` modifier: `internal`, `private`,
  `fileprivate` and the default. So an `XCTestCase` class and its test methods
  stay silent, and so does a `func` inside a `public extension` that carries no
  modifier of its own.
- An `open` declaration. The rule documents itself as "public or open", and the
  code reads `.public` alone. Measured: `open class Base { open func m() {} }`
  reports nothing. This is a gap, and no other rule of this set covers it.
- A `public override` member. The code skips every declaration that carries
  `override`.
- An enum case, and a requirement of a protocol.

The `missing-docs.md` prompt rule states which of its carve-outs this gate
keeps. The gate keeps neither the getter carve-out nor the obvious-
implementation carve-out for Swift: it reports what the list above says and
nothing else.

## How to exempt one declaration

Write the directive in the code, where a reader sees it:

| the directive | where it stands | reported |
|---|---|---|
| `// swift-format-ignore: AllPublicDeclarationsHaveDocumentation` | the line above the declaration | NO |
| `// swift-format-ignore` | the line above the declaration | NO |
| `// swift-format-ignore-file` | the top of the file | NO, for every declaration |
| `// swift-format-ignore` | after the declaration, on the same line | YES — a trailing comment does nothing |

Each line form covers ONE declaration. The file form takes the whole file out,
and it is the way generated code leaves the gate: the `missing-docs` prompt rule
carves generated code out, and the tool carries no generated-code check of its
own. A rule name the tool does not know is accepted and silences nothing.

## What the fixture pair holds

The failing fixture holds one undocumented public structure, one undocumented
public method and one undocumented public function, beside one documented
function. Measured with the shipped script: **3 findings**, exit 0.

The passing fixture documents every public declaration. Measured with the
shipped script: 0 findings, exit 0. The raw run over it writes two
`[Indentation]` lines, and the filter drops both, so the doctor's own fixture
pair measures the filter as well as the rule.

The doctor materializes the fixture directory flat and hands this rule one
path, so neither fixture reads the other.
