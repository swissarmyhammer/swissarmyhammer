---
name: idioms-swift
description: Swift declarations written in a non-preferred form when an equivalent preferred form exists — checked by the toolchain's own `swift format`, not by prompt.
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
    work="$(mktemp -d)"
    trap 'rm -rf "$work"' EXIT
    probe=sah-probe.swift
    printf '%s\n' AlwaysUseLiteralForEmptyCollectionInit \
      DontRepeatTypeInStaticProperties NoVoidReturnOnFunctionSignature \
      ReplaceForEachWithForLoop UseLetInEveryBoundCaseVariable \
      UseShorthandTypeNames UseSynthesizedInitializer | sort -u > "$work/allowed"
    allowed="$(paste -s -d'|' - < "$work/allowed")"
    if [ -z "$allowed" ]; then
      printf 'idioms-swift: the tag allowlist is empty, so no file was judged\n' >&2
      exit 1
    fi
    {
      printf '{"rules":{'
      sed -e 's/.*/"&":true/' "$work/allowed" | paste -s -d, -
      printf '}}\n'
    } > "$work/rules.json"
    for file in "$@"; do
      if [ ! -e "$file" ]; then
        printf 'sah-diagnostic: idioms-swift found no file at %s, so its declarations are unread\n' "$file" >&2
        continue
      fi
      if [ ! -f "$file" ] || [ ! -r "$file" ]; then
        printf 'sah-diagnostic: idioms-swift declined %s: the path is not a readable file\n' "$file" >&2
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
          total = split(allowed, names, "|")
          for (i = 1; i <= total; i++) keep[names[i]] = 1
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
          if (tag in keep) {
            print file ":" part[1] ": " tag ": " substr(mark, shut + 2) > kept
          }
        }' "$work/lint.err"
      trouble="$(sed -n '1p' "$work/refused")"
      if [ -n "$trouble" ] || { [ "$status" -ne 0 ] && [ "$status" -ne 1 ]; }; then
        printf 'sah-diagnostic: idioms-swift declined %s: %s\n' \
          "$file" "${trouble:-swift format exited $status}" >&2
        continue
      fi
      cat "$work/kept"
    done
  doctor:
    check_command: "which swift mktemp awk cat rm sed sort paste && printf '' | swift format lint --strict --configuration '{}' --assume-filename sah-probe.swift -"
    check_version_command: "swift --version"
    fix_hint: "install the Swift toolchain, Swift 6.2 or newer — Xcode 26 and above ship it"
---

# Idioms — Swift

`swift format` decides, in one run, a set of Swift idioms a reader would
otherwise judge by eye. Each rule this gate keeps names one shape and asks for
an equivalent shape, so the tool's answer is a fact about the source rather
than a preference: the two forms compile to the same program, and the tool
names which one the project writes.

The gate runs `swift format lint`. It never rewrites a file. Review is
read-only, so a finding names the line and the author makes the edit.

`swift format` is a SUBCOMMAND of the Swift toolchain, spelled with a space. It
is part of the Swift toolchain. This gate runs the toolchain, so it needs no
install step at all.

Every measurement below was made with the toolchain of this machine and of the
self-hosted macOS CI runner:

    Apple Swift version 6.4 (swiftlang-6.4.0.33.1 clang-2100.3.33.1)
    Target: arm64-apple-macosx27.0.0

`swift format --version` writes `main`, so it names no release. The Swift
version above is the only version a measurement can carry. The binary stands at
`/Applications/Xcode-beta.app/Contents/Developer/Toolchains/XcodeDefault.xctoolchain/usr/bin/swift-format`.

`swift format dump-configuration` writes the configuration the tool would read.
It holds 43 keys under `rules`, and each key is one rule.

## Which rules the gate keeps

Seven of the 43 decide an IDIOM. The script names them in one list, and that
one list does two jobs: it writes the configuration that turns the seven ON,
and it is the allowlist the output filter reads.

| the rule | what it decides |
|---|---|
| `AlwaysUseLiteralForEmptyCollectionInit` | the empty `Array` and `Dictionary` literal (`[Int]()` becomes `: [Int] = []`, `[String: Int]()` becomes `: [String: Int] = [:]`), and nothing for `Set`. It is OFF in the default configuration, so the written configuration is what turns it ON |
| `DontRepeatTypeInStaticProperties` | the repeated type name in a static member, for one shape of many |
| `NoVoidReturnOnFunctionSignature` | the `Void` return clause, BOTH halves: `-> Void` and `-> ()` on a function signature |
| `ReplaceForEachWithForLoop` | the `forEach` walk: every `.forEach { ... }` call with a closure, whether the closure holds an `if` or a `filter` chain feeds it. `forEach(print)` with a function reference is silent |
| `UseLetInEveryBoundCaseVariable` | a `let` on each bound case variable (`case .p(let m, let n)`), not one `let` on the pattern: `case let .p(m, n)` is reported and `case .p(let m, let n)` is silent |
| `UseShorthandTypeNames` | the shorthand type sugar: `[Int]`, `[K: V]` and `Int?` in place of `Array<Int>`, `Dictionary<K, V>` and `Optional<Int>` |
| `UseSynthesizedInitializer` | the redundant memberwise initializer, the INTERNAL half alone |

`the_shipped_swift_idioms_tool_rule_names_only_rules_swift_format_knows` holds
each of the seven to standing in the `rules` table of
`swift format dump-configuration`, so a misspelling and a rule the toolchain
renames each fail that test by name. A misspelled tag would otherwise match no
output line, and a tag that matches nothing looks exactly like a clean file.

The gate keeps NONE of `NeverForceUnwrap`, `NeverUseForceTry` and
`NeverUseImplicitlyUnwrappedOptionals`. `disallowed-constructs-swift` owns those
three bullets: it runs swiftlint, which carries the test-target split and the
`ignored_literal_argument_functions` option that `swift format` does not have.
One requirement takes one owner.

`UseWhereClausesInForLoops` is not in the allowlist: the tool cannot tell a loop
that acts from a loop that builds a collection.

## The allowlist is the gate, and the configuration only a second one

The `rules` table the script writes REPLACES the default table. Measured with
Apple Swift 6.4 over `let Bad_One = 1`:

| the configuration | reported |
|---|---|
| none | `[AlwaysUseLowerCamelCase] rename the constant 'Bad_One' using lowerCamelCase` |
| a `rules` table naming `AlwaysUseLiteralForEmptyCollectionInit` alone | nothing |

So one small JSON file states the whole rule set, and the 36 rules the
allowlist does not name are OFF. Every other key of the configuration —
`lineLength`, `indentation` and the rest — keeps its default, because the file
states none of them.

That configuration still is not the gate. NINE tags come from two parts of the
tool that stand beside the 43 rules, and no key reaches either part. The table
under this one measures each of the nine. So the script reads the `[<Name>]`
tag off each output line and KEEPS ONLY a tag the allowlist names.
`the_shipped_swift_idioms_tool_rule_drops_every_tag_outside_its_allowlist`
drives one probe for each of the nine through the shipped script and holds each
run to zero findings, so a script that dropped the filter reports layout there
and fails by name.

## The tags no configuration can stop

`swift format lint` writes one line for each finding, and each line carries a
`[<Name>]` tag. NINE of those tags come from two parts of the tool that stand
beside the 43 rules. The PRETTY-PRINTER lays each file out again, and the
WHITESPACE LINTER compares the file with that layout. Both parts run whatever
the configuration says, and no key of the configuration reaches either one.

Each row below was measured over its own probe file, with every one of the 43
rules switched OFF:

    swift format dump-configuration > all-off.json    # then each rule set to false
    swift format lint --strict --configuration all-off.json <probe>.swift

| the tag | what draws it | what the run wrote |
|---|---|---|
| `AddLines` | two members on the brace line of a `struct` | `add 1 line break`, at 1:15 and at 2:16 |
| `EndOfLineComment` | a trailing comment that runs past column 100 | `move end-of-line comment that exceeds the line length` |
| `Indentation` | a member indented 8 spaces where 2 are asked for | `unindent by 6 spaces` |
| `LineLength` | a declaration that runs past column 100 | `line is too long` |
| `RemoveLine` | four blank lines between two declarations | `remove line break`, at 1:14 and at 2:1 |
| `Spacing` | `let alpha = 1+2` | `add 1 space`, at 1:14 and at 1:15 |
| `SpacingCharacter` | a TAB between the `=` and the value, in `struct A {` then `  let x =<TAB>1` then `}` | `use spaces for spacing`, at 2:10 |
| `TrailingComma` | an array literal over several lines, with no trailing comma | `add trailing comma to the last element in multiline collection literal` |
| `TrailingWhitespace` | three spaces after a declaration | `remove trailing whitespace` |

### Where the nine names come from

The nine names are the CASES of two enumerations of the tool. A case reaches
the output with its first letter in upper case, so `spacingCharacter` is
written as `[SpacingCharacter]`.

- `PrettyPrintFindingCategory` holds TWO cases: `endOfLineComment` and
  `trailingComma`. The PRETTY-PRINTER writes them.
- `WhitespaceFindingCategory` holds SEVEN cases: `trailingWhitespace`,
  `indentation`, `spacing`, `spacingCharacter`, `removeLine`, `addLines` and
  `lineLength`. The WHITESPACE LINTER writes them.

`strings` cannot answer this question, and an earlier version of this section
gave a wrong reason for that. Each name has TWO forms, and both of them were
measured on the Apple Swift 6.4 binary. The number is what `grep -cx` writes:

    B="$(xcrun --find swift-format)"
    strings -a "$B" | grep -cx <name>

    addLines             1   AddLines             0
    indentation          5   Indentation          0
    lineLength           2   LineLength           0
    removeLine           1   RemoveLine           0
    spacing              1   Spacing              0
    trailingComma        1   TrailingComma        0
    endOfLineComment     1   EndOfLineComment     1
    spacingCharacter     1   SpacingCharacter     1
    trailingWhitespace   1   TrailingWhitespace   1

So EVERY one of the nine CASE names stands in the binary as data. The earlier
version of this section said they did not, and that is the sentence this one
corrects.

The TAG form is a second string, and `strings` finds three of the nine. Those
three are exactly the three that are 16 bytes or longer. A name of 15 bytes or
fewer is invisible to `strings` whether the tool holds it or not, because Swift
keeps a string literal of that size inside the instruction stream and not in a
data section. A two-line probe measures that limit by itself:

    func fifteen() -> String { return "AAAAAAAAAAAAAAA" }
    func sixteen() -> String { return "BBBBBBBBBBBBBBBB" }
    print(fifteen(), sixteen())

    xcrun swiftc -O Probe.swift -o Probe
    strings -a Probe | grep -cx 'AAAAAAAAAAAAAAA'    # writes 0
    strings -a Probe | grep -cx 'BBBBBBBBBBBBBBBB'   # writes 1

So the number `strings` writes for a TAG form measures the LENGTH of that name,
and not whether the tool knows it.

Neither number answers the question this section asks, and the length is not
why. `strings` writes a run of bytes and it NEVER writes the type that owns the
run. The name `indentation` stands five times in the binary, and no line of the
output says which of the five is a case of `WhitespaceFindingCategory`. The
question is WHICH names are the finding categories, so the answer must come
from the metadata that holds a name beside its type.

That metadata is the `__swift5_fieldmd` section. It carries one field
descriptor for each type of the image, and a descriptor names its own type and
points at the name of each of its cases. This command reads them:

    python3 cases.py "$(xcrun --find swift-format)"

`cases.py` walks the field descriptor of each type of the image, and writes the
cases of every type whose name ends in `FindingCategory`:

    import struct, sys
    b = open(sys.argv[1], "rb").read()
    sects, o = [], 32
    for _ in range(struct.unpack_from("<I", b, 16)[0]):
        cmd, size = struct.unpack_from("<II", b, o)
        if cmd == 0x19:
            for i in range(struct.unpack_from("<I", b, o + 64)[0]):
                s = o + 72 + i * 80
                sects.append(struct.unpack_from("<QQI", b, s + 32) + (b[s : s + 16].rstrip(b"\0"),))
        o += size
    at = lambda a: next(f + a - v for v, n, f, _ in sects if v <= a < v + n)
    cs = lambda a: b[at(a) : b.index(b"\0", at(a))]
    rel = lambda a: a + struct.unpack_from("<i", b, at(a))[0]
    vm, size, off, _ = next(s for s in sects if s[3] == b"__swift5_fieldmd")
    p = 0
    while p < size:
        d = vm + p
        name, _sup, _kind, rs, n = struct.unpack_from("<iiHHI", b, off + p)
        t = cs(d + name)
        if t[:1] == b"\x01":
            t = cs(rel(rel(d + name + 1) + 8))
        if t.endswith(b"FindingCategory"):
            print(t.decode(), [cs(rel(d + 16 + i * rs + 8)).decode() for i in range(n)])
        p += 16 + n * rs

Measured with Apple Swift 6.4, it writes three lines:

    RuleBasedFindingCategory ['ruleType']
    PrettyPrintFindingCategory ['endOfLineComment', 'trailingComma']
    WhitespaceFindingCategory ['trailingWhitespace', 'indentation', 'spacing', 'spacingCharacter', 'removeLine', 'addLines', 'lineLength']

Those three are EVERY type of the binary whose name ends in `FindingCategory`.
The third one, `RuleBasedFindingCategory`, carries the name of the RULE that
reported, so its tags are the 43 rule names and a configuration key stops each
one. The other two carry the nine tags of the table above, and no key reaches
them.

### What that costs a gate

So the configuration is only a SECOND gate. This gate reads the tag off each
output line, and it KEEPS only a tag that stands in the allowlist the script
states. A gate that kept every line would report layout.

`the_shipped_swift_idioms_rule_body_names_every_tag_swift_format_writes` holds
this table, and it reads THREE sources. It reads the tag column out of this
body. It drives one probe for each tag through the live toolchain, so a release
that stops writing a tag fails that test BY NAME. And it reads the case set of
`PrettyPrintFindingCategory` and `WhitespaceFindingCategory` out of the
reflection metadata of the toolchain binary, so a tag the tool owns and this
table does not name fails that test as well. The third source is the necessary
one: a person wrote this table and the probe list, and while the test held only
those two to each other, `SpacingCharacter` stood in neither and broke no test.

## What the shipped passing fixture draws from the pretty-printer

The leak is not a corner case. Both fixtures of this rule are written with
4-space indentation, which is the house style of every Swift fixture this set
ships, and `swift format` asks for 2. Measured over the shipped passing
fixture:

| the run | findings |
|---|---|
| `swift format lint --strict` with no filter | 34 lines, every one of them `[Indentation]` |
| the shipped script | 0 findings, exit 0 |

So the filter is load-bearing in the doctor's own fixture pair. A gate with no
allowlist would fail this rule's own passing fixture.

## Every status the lint run writes

`swift format lint --strict` was run over each shape below. The run states no
configuration, so it reads the 43 rules as shipped:

    swift format lint --strict Probe.swift

| the run | status | stdout | stderr |
|---|---|---|---|
| a file with findings | 1 | 0 bytes | `Probe.swift:1:14: error: [Spacing] add 1 space`, twice |
| a file with no finding | 0 | 0 bytes | 0 bytes |
| a path that holds no file | 0 | 0 bytes | 0 bytes |
| a file with no read permission | 1 | 0 bytes | `<unknown>: error: Unable to open Probe.swift: file is not readable or does not exist` |
| a file whose bytes are not UTF-8 | 1 | 0 bytes | `<unknown>: error: Unable to lint Probe.swift: file is not readable or does not exist.` |
| a file the parser cannot read | 1 | 0 bytes | `Probe.swift:2:4: error: expected name in attribute`, three times |
| a path that names a directory | 64 | 0 bytes | `Error: 'Probe.swift' is a path to a directory, not a Swift source file.` |

Three answers in that table decide how the script is shaped.

**Row 3 is the dangerous one.** A path that holds no file exits 0 and writes
NOTHING. That reads exactly like a clean pass over a file the run never opened.
The script therefore tests the path itself before it starts the tool, with
`[ ! -e "$file" ]`, and writes its own marked line for it. The tool gives no
error to read.

The script tests the path a SECOND time, with
`[ ! -f "$file" ] || [ ! -r "$file" ]`, because it hands the source to the tool
on standing input. Rows 4 and 7 would otherwise fail at the REDIRECT rather
than in the tool, and a redirect that fails writes the shell's own words on the
channel the engine reads. Those two tests answer rows 3, 4 and 7 before the
tool starts, and row 5 stays with the tool, which answers it over standing
input with `<unknown>: error: Unable to lint sah-probe.swift: file is not
readable or does not exist.` at status 1. Measured with the shipped script,
each of the five refusing paths staged beside a file that holds one finding:
every run reported that finding, wrote one marked line, and exited 0.

**Every line stands on STDERR.** Findings and errors share that one channel, and
stdout holds 0 bytes in every row. The script reads stderr. A gate that read
stdout would read an empty channel for a file that holds findings.

**One status carries a finding and an error alike.** Rows 1, 4, 5 and 6 all
exit 1, so the status cannot tell a judged file from a refused one. The script
therefore reads the SHAPE of each stderr line: a line whose FIRST
`sah-probe.swift:` carries `<line>:<column>: error: [<Tag>] ` right after it is
a tagged finding, and every other line is trouble. A file that wrote a trouble
line is declined whole, because a file the parser could not read is a file no
rule judged. The section under this one states why the reading finds that head
by its POSITION, and why neither the name of the file nor the bytes in it can
move that position.

`--strict` is what makes those lines say `error:`. Without it the same findings
arrive as `warning:` and the run exits 0, and the filter would match nothing.

`the_shipped_swift_idioms_rule_body_records_every_status_the_lint_run_writes`
holds this table. It reads the run column and the status column out of this
body, and it drives each shape through the live toolchain.

## The reading finds the head of a diagnostic by its POSITION

A line `swift format` writes carries TWO parts a person can choose. The tool
writes the PATH FIRST on every line, so the NAME of the file stands at the head.
And it writes the SOURCE TEXT of a parse error inside the message, so the BYTES
of the file stand in the middle. Three readings of such a line are therefore
wrong, and this rule shipped each of them once:

- A LOOSE reading matches its text at ANY position. It reads the name of the
  file as well, so a file NAME can give the pattern the reading looks for.
- An ANCHORED reading writes the path INTO the pattern, to hold the match at
  the head of the line. The path then has to pass through a regular expression
  and through a substitution, and a legitimate name that does not pass through
  both costs its file every finding it drew.
- A LAST-HEAD reading walks to the LAST `:<line>:<column>: error: ` of the
  line. It names the path nowhere, so no NAME reaches it. But the message
  quotes the source AFTER the true head, so the BYTES of a file can put a
  second head where that reading stops.

The shipped reading takes both parts out of the file's hands, and it takes them
in that order.

**The name goes first.** The script hands the tool the SOURCE on standing
input, under one assumed name that holds no `:`:

    swift format lint --strict --configuration "$work/rules.json" \
      --assume-filename sah-probe.swift - < "$file"

The tool then writes `sah-probe.swift:<line>:<column>: error: ...` whatever the
file is named. Measured over a file named
`w: error: [UseShorthandTypeNames] s.swift` holding one `Array<Int>` parameter,
the one output line reads `sah-probe.swift:2:39: error: [UseShorthandTypeNames]
use shorthand syntax for this 'Array' type`, so the name reaches no line at all.

That pair carries a version cost, and the section "The Swift version floor, and
the version CI runs" states it: `--assume-filename` beside the path `-` is
accepted only at Swift 6.2 and above, so the floor of this gate stands above
the Swift 6.0 the `swift format` subcommand alone needs.

**The bytes go second.** One `awk` reads the FIRST `sah-probe.swift:` of the
line and asks for the head immediately after it:

    at = index($0, opening)
    rest = at > 0 ? substr($0, at + size) : ""
    if (at == 0 || match(rest, /^[0-9]+:[0-9]+: error: /) == 0) {
      print $0 > refused
      next
    }

Nothing the file holds can stand BEFORE the path the tool wrote, so the FIRST
head is the true head and no content can move it. What stands after that head
decides the line: a `[<Tag>] ` makes the line a finding of that tag, and the
script keeps the line only where the allowlist names the tag. Every other line
is trouble, and a file that wrote one is declined whole. So ONE reading answers
the decline guard and the tag filter together, and the two cannot disagree.

The finding the script writes carries the path the ARGUMENT LIST spelled, and
the line number off the head. The path therefore reaches no pattern, no
replacement and no line of the tool, so neither the SPELLING of a name, nor the
CHARACTERS in it, nor the BYTES of the file can reach the reading. `awk` is the
one tool this reading adds, and `doctor.check_command` names it beside `swift`,
`mktemp`, `cat`, `rm`, `sed`, `sort` and `paste`, which are every other utility
the run block calls.
`the_shipped_swift_idioms_rule_doctor_names_every_utility_its_script_runs`
holds that list to the script in BOTH directions.

Standing input costs the run nothing else. Measured over the shipped fixture
pair with Apple Swift 6.4: the failing fixture reports the same 11 findings
carrying the same 7 rules as a run that named the path, line for line, and the
passing fixture reports 0. A run over standing input reads the same rule set,
obeys the same `--configuration`, and reads the same `// swift-format-ignore`
directives.

### What each reading answers over a file NAME

Each row below was measured with Apple Swift 6.4, BSD sed, BSD grep
2.6.0-FreeBSD and BSD awk 20200816, under `LC_ALL=en_US.UTF-8`. The loose
reading is the first shape of this run, with the guard spelled
`grep -v ': error: \['` and the filter spelled
`grep -E ": error: \[($allowed)\] "`. The anchored reading is the second, with
both readings anchored on `^$quoted:[0-9]+:[0-9]+: error: ` and with the rewrite
an `s` command delimited by `|`. The last-head reading is the third, with the
`awk` walking to the last head of a run that named the path:

| the file name | what the file holds | the loose reading | the anchored reading | the last-head reading | the shipped script |
|---|---|---|---|---|---|
| `Plain.swift` | one member indented 8 spaces | 0 findings | 0 findings | 0 findings | 0 findings |
| `x: error: [UseShorthandTypeNames] y.swift` | the same member | 1 finding — `Indentation: unindent by 6 spaces` | 0 findings | 0 findings | 0 findings |
| `yy:1:1: error: [UseShorthandTypeNames] y.swift` | the same member | 1 `Indentation` finding | 0 findings | 0 findings | 0 findings |
| `PlainBroken.swift` | Swift the parser cannot read, with no payload | 0 findings, 1 marked line | 0 findings, 1 marked line | 0 findings, 1 marked line | 0 findings, 1 marked line |
| `z: error: [UseShorthandTypeNames] q.swift` | the same bytes | the raw `error: expected name in attribute` lines as findings, and NO marked line | 0 findings, 1 marked line | 0 findings, 1 marked line | 0 findings, 1 marked line |
| `zz:1:1: error: [UseShorthandTypeNames] q.swift` | the same bytes | 1 finding, a raw parser line the `sed` rewrote in part, and NO marked line | 0 findings, 1 marked line | 0 findings, 1 marked line | 0 findings, 1 marked line |
| `w: error: [UseShorthandTypeNames] s.swift` | one `Array<Int>` parameter | 1 `UseShorthandTypeNames` finding | 1 `UseShorthandTypeNames` finding | 1 `UseShorthandTypeNames` finding | 1 `UseShorthandTypeNames` finding |
| `café.swift` | the same parameter | 1 `UseShorthandTypeNames` finding | 0 findings, 1 marked line | 1 `UseShorthandTypeNames` finding | 1 `UseShorthandTypeNames` finding |
| `ünïcodé.swift` | the same parameter | 1 `UseShorthandTypeNames` finding | 0 findings, 1 marked line | 1 `UseShorthandTypeNames` finding | 1 `UseShorthandTypeNames` finding |
| `日本語.swift` | the same parameter | 1 `UseShorthandTypeNames` finding | 1 `UseShorthandTypeNames` finding | 1 `UseShorthandTypeNames` finding | 1 `UseShorthandTypeNames` finding |
| `a\|b.swift` | the same parameter | 1 `UseShorthandTypeNames` finding | 1 RAW tool line, unrewritten | 1 `UseShorthandTypeNames` finding | 1 `UseShorthandTypeNames` finding |

Rows 2 and 3 are WRONG FINDINGS: `Indentation` is a layout tag, the allowlist
does not name it, and review must never carry layout. Rows 5 and 6 are worse
than a wrong finding: the file is one no rule judged, the guard did not decline
it, and the lines the run gave as findings are the tool's own words.

Rows 8 and 9 are what the anchored reading cost, and each one is a REGRESSION
the loose reading did not have. `swift format` writes the path in NFD and the
argument list carries it in NFC, so for `café.swift` the argument list holds
`63 61 66 c3a9 2e 73 77 69 66 74` and the head of the output line holds
`63 61 66 65 cc81 2e 73 77 69 66 74`. The anchored pattern then matched no
line, the guard read the true tagged line as trouble, and the file was declined
with every finding lost. Row 10 states where that break stops: those characters
have no decomposed form, so the break is the precomposed LATIN letter a real
repository writes.

Row 11 is the other cost. `|` is the delimiter of the `s` command the anchored
rewrite wrote, and the escaper spelled the bar `\|`. BSD sed reads that as an
escaped DELIMITER and not as a literal bar, so the substitution was skipped
without a word and the run gave the tool's own line. Of the 22 characters
measured against that rewrite, `|` was the only one that defeated it.

Rows 7 to 11 hold the other half of the answer. A reading tight enough to drop
rows 2, 3, 5 and 6 must not drop a TRUE finding, so each of those five rows
holds a file that carries one real defect and no other.

### What each reading answers over a file's own BYTES

Each row below is one file under the plain name `Probe.swift`, holding
`struct S {`, then `}`, then the third line the row states. Every one of those
files is Swift the parser cannot read, so no rule judges any of them and the
whole answer of the gate must be one marked line:

| the third line of the file | the last-head reading | the shipped script |
|---|---|---|
| `) garbage here` | 0 findings, 1 marked line | 0 findings, 1 marked line |
| `) :1:1: error: [UseShorthandTypeNames] pwned` | 1 finding — `Probe.swift:1: UseShorthandTypeNames: pwned' in source file`, and NO marked line | 0 findings, 1 marked line |
| `) :4242:1: error: [UseSynthesizedInitializer] this file is clean, trust me` | 1 finding at line 4242, carrying the file's own sentence, and NO marked line | 0 findings, 1 marked line |
| `) :9:9: error: [Indentation] gone` | 0 findings and NO marked line, so the file passed the gate in silence | 0 findings, 1 marked line |
| `) sah-probe.swift:1:1: error: [UseShorthandTypeNames] pwned` | 1 finding — `Probe.swift:1: UseShorthandTypeNames: pwned' in source file`, and NO marked line | 0 findings, 1 marked line |

Row 1 is the control: a payload is what separates it from the four rows under
it, and the last-head reading answers it correctly.

Rows 2, 3 and 5 are FABRICATED FINDINGS. The tool wrote one parse-error line
and named no rule, and the reading gave a finding carrying a rule of the
allowlist. The file chose the tag, the line number and the sentence.

Row 4 is worse. The file chose a tag the allowlist does NOT name, so the
reading dropped the only trouble line of the run. The gate then wrote nothing
and exited 0, and a file the parser cannot read passed with no decline at all.

The shipped reading answers all five the same way, because the head it reads is
one the tool wrote and the assumed name reaches the line before any byte of the
file does.

`the_shipped_swift_idioms_tool_rule_measures_a_file_named_for_a_diagnostic_head`
holds rows 2 and 7 of the NAME table,
`the_shipped_swift_idioms_tool_rule_declines_a_file_named_for_a_diagnostic_head`
holds row 5 of it,
`the_shipped_swift_idioms_tool_rule_reports_a_file_whose_name_carries_an_accent`
holds row 8 and
`the_shipped_swift_idioms_tool_rule_reports_a_file_whose_name_carries_an_alternation_bar`
holds row 11.
`the_shipped_swift_idioms_tool_rule_measures_a_file_whose_source_forges_a_diagnostic_head`
holds rows 2 and 3 of the BYTES table, and
`the_shipped_swift_idioms_tool_rule_declines_a_file_whose_source_forges_a_diagnostic_head`
holds row 4.

## Why the script runs `swift format` once for each file

A path that names a DIRECTORY costs a SHARED run everything. Measured with
`Dirty.swift`, which holds one finding, beside each refusing path in ONE run:

| the refusing path | status | findings | `Dirty.swift` judged |
|---|---|---|---|
| a path that holds no file | 1 | 1 | yes |
| a file with no read permission | 1 | 1 | yes |
| a file whose bytes are not UTF-8 | 1 | 1 | yes |
| a file the parser cannot read | 1 | 1 | yes |
| a path that names a directory | 64 | 0 | NO |

`builtin/validators/README.md` refuses that last row in as many words: "A
nonzero exit fails the WHOLE run, so one unjudged path throws away every finding
the run did make." A work list reaches a directory whenever a project holds a
`.swift` directory name, so the row is not a corner case a gate may accept.

Two more facts of a shared run point the same way. The stderr of one run mixes
the lines of every path, and an `<unknown>: error:` line names its file only
inside the sentence, so a script cannot attribute a trouble line to a path by
reading its head. And a file the parser cannot read writes an ABSOLUTE path
where every other line writes the path the work list held.

So the script hands `swift format` ONE path for each run. A refusing path then
costs its own file and nothing more, and the script writes one line opening
`sah-diagnostic:` that names the path. That line carries the tool's own words
where the tool answered, and the script's own words where a guard answered
before the tool started: a path that holds no file and a path that is not a
readable file are both the script's answer, because the tool reads the SOURCE
on standing input and never opens the path itself. Measured with the shipped
script over the five refusing paths above, each staged beside a file that holds
one finding: the run reported that finding every time, wrote one marked line
every time, and exited 0 every time.

`the_shipped_swift_idioms_tool_rule_reports_a_file_beside_one_it_declined`
holds both halves of that, and
`the_shipped_swift_idioms_tool_rule_reports_every_file_that_reports` holds the
loop to keeping the findings of two files that both report — which is what
`set -e` would take away, because a file with findings exits 1.

## What the project's own `.swift-format` decides, and what it cannot

`swift format` reads a project `.swift-format` on its own, and the
`--configuration` the script writes takes that file out of the run. Measured
over one file holding `public var items = [Int]()`, beside a project
`.swift-format` holding `"AlwaysUseLiteralForEmptyCollectionInit": false`:

| the run | findings |
|---|---|
| the shipped script | 1 |

So the project cannot turn a rule of this gate off. To exempt one declaration,
the author writes the directive the section below states, in the code, where a
reader sees it.

## `DontRepeatTypeInStaticProperties` reports one shape

The rule DOES report. An earlier probe found it silent, because that probe named
a property of type `Int`.

Each row below is a `public struct Color` that holds one member, over three
lines, with that one rule ON:

| the declaration | reported |
|---|---|
| `public static let redColor = Color()` | YES — `remove the suffix 'Color' from the name of the variable 'redColor'` |
| `public static let redColor = 1` | NO |
| `public static let redColor: Color = Color()` | NO |
| `public static var redColor: Color { Color() }` | NO |
| `public static let colorRed = Color()` | NO |
| `public static let red = Color()` | NO |
| `public static func makeColor() -> Color { Color() }` | NO |
| `public static let bluePalette = Palette()`, in `public enum Palette` | YES |
| `public static let darkTheme = Theme()`, in `public final class Theme` | YES |
| `public static let greenColor = Color()`, in `extension Color` | YES |
| `static var redColor: Color { get }`, in `public protocol Color` | NO |

The rule reads three facts at one time. The member must be `static` or `class`.
Its TYPE must be the type that holds it, and the rule reads that type from an
initializer that CALLS the type. And the member name must end in the type name
as a SUFFIX: `colorRed` and `redColor2` each pass.

An annotation makes the rule silent when an initializer stands beside it. Row 3
holds `redColor: Color = Color()` and draws nothing, and row 1 holds the same
member with the annotation off and reports.

Two more shapes were measured, and neither stands in the table, because Swift
compiles neither. `struct Sandwich { static let bolognaSandwich: Sandwich }`
written over three lines reports
`remove the suffix 'Sandwich' from the name of the variable 'bolognaSandwich'`,
and the same declaration written on ONE line draws nothing. `swiftc -typecheck`
refuses both: `'static let' declaration requires an initializer expression or an
explicitly stated getter`.

So the shapes the rule MISSES are these: a member the author annotates, a member
whose type is inferred from a literal of another type, a computed member, a
member whose name carries the type name anywhere but at its end, and a
requirement of a protocol. `idioms.md` therefore KEEPS the type-name bullet.
The bullet states `static let redColor` on `Color` and it names no type, so the
tool decides one shape of many.
`the_shipped_swift_idioms_tool_rule_reads_one_shape_of_type_name_repetition`
holds row 1 and row 2 together, so a release that moved either one fails by
name.

## `UseSynthesizedInitializer` reads the access level

Each row below is a `struct Point` that holds `var x: Int` and `var y: Int`,
beside one explicit memberwise initializer, with that one rule ON:

| the initializer | reported |
|---|---|
| an `internal` `init` of an `internal` struct | YES |
| a `public` `init` of a `public` struct | NO |
| an `internal` `init` of a `public` struct | YES |
| a `private` `init` of an `internal` struct | NO |
| an `init` whose parameter carries a default value | YES |
| a `public` `init` of a `public` struct that holds `internal` properties | NO |
| an `init` whose body writes `self.x = max(0, x)` | NO |

The message reads `remove this explicit initializer, which is identical to the
compiler-synthesized initializer`.

The silent half is correct, and it is not a hole. Swift synthesizes an INTERNAL
memberwise initializer, never a `public` one, so a `public init` is not identical
to it. Deleting a `public init` takes a declaration off the package surface.

The rule is therefore the OWNER of the internal half and of nothing more. A
prompt bullet that wants the public half must state it in words, and it must
state that half alone: a bullet that named both halves would fight the tool on
every review round.
`the_shipped_swift_idioms_tool_rule_reads_the_internal_synthesized_initializer`
holds row 1 and row 2 together.

## Which rule reports a filter chain

A `filter` chain that feeds a `forEach` is this walk:

```swift
things.filter { $0 > 2 }.forEach { thing in print(thing) }
```

Measured over one file for each walk, `ReplaceForEachWithForLoop` ON alone:

| the walk | `ReplaceForEachWithForLoop` |
|---|---|
| `things.filter { $0 > 2 }.forEach { thing in print(thing) }` | YES, at 2:28 |
| `things.forEach { if $0 > 2 { print($0) } }` | YES, at 2:10 |
| `for thing in things where thing > 2 { print(thing) }` | NO |

`ReplaceForEachWithForLoop` reports the chain, and it is in the allowlist, so
the gate reports it. Its message reads `replace use of '.forEach { ... }' with
for-in loop`.

The shape its fix lands on is NOTHING. Measured: `swift format --in-place` with
that rule ON leaves the file byte for byte as it was. The rule REPORTS and it
CORRECTS nothing, so the message is the whole of what the author receives.

`the_shipped_swift_idioms_tool_rule_reports_a_filter_chain` holds row 1.

## The directive an author writes

The directive is `// swift-format-ignore`. Each row below is one file holding
`let Bad_One = 1`, with `AlwaysUseLowerCamelCase` ON:

| the directive | where it stands | reported |
|---|---|---|
| `// swift-format-ignore` | the line above | NO |
| `// swift-format-ignore: AlwaysUseLowerCamelCase` | the line above | NO |
| `// swift-format-ignore-file` | the top of the file | NO, for every declaration |
| `// swift-format-ignore-file: AlwaysUseLowerCamelCase` | the top of the file | NO, for every declaration |
| `// swift-format-ignore` | above the first of two declarations | the SECOND declaration alone |
| `// swift-format-ignore` | after the declaration, on the same line | YES — a trailing comment does nothing |

So both forms work, and each covers ONE declaration. The line form is
`// swift-format-ignore` on the line above the declaration, and
`// swift-format-ignore: <RuleName>` names one rule. The rule name to write is
the name the finding carries, because the script writes `<RuleName>: <reason>`
as the finding text. The file form is `// swift-format-ignore-file` at the top,
and it takes the whole file out.

A rule name the tool does not know is accepted, and it silences nothing.
Measured: `// swift-format-ignore: NoSuchRuleName` above `let Bad_One = 1` still
reports.

The directive reaches the pretty-printer as well, and only in its bare form. Each
row below is a file with a member indented 8 spaces, with all 43 rules OFF:

| the directive | the `[Indentation]` finding |
|---|---|
| none | YES |
| `// swift-format-ignore` above the declaration | NO |
| `// swift-format-ignore: Indentation` above the declaration | YES |
| `// swift-format-ignore-file` at the top | NO |

A tag is not a rule name, so the named form cannot reach one. That costs this
gate nothing, because the filter drops every layout tag before a report is
written; it matters to an author who runs `swift format lint` by hand.

## The Swift version floor, and the version CI runs

The floor of this GATE is **Swift 6.2**, and the OPTION PAIR the run block gives
the tool sets it. The `swift format` SUBCOMMAND arrived earlier, at Swift 6.0,
and no rule of the allowlist sets a floor at all, because each shipped long
before the subcommand did. The gate asks more of the tool than the subcommand
alone, so its floor stands above both.

| the question | how it was measured | the answer |
|---|---|---|
| when `UseWhereClausesInForLoops` first shipped | `Sources/SwiftFormatRules/UseWhereClausesInForLoops.swift` on `release/5.6` | HTTP 200, so Swift 5.6 or older |
| when `AlwaysUseLiteralForEmptyCollectionInit` first shipped | the same directory on `release/5.9`, then `Sources/SwiftFormat/Rules/` on `release/5.10` | HTTP 404, then HTTP 200, so Swift 5.10 |
| when `swift format` first ran | the `README.md` of `release/5.10` and of `release/6.0` | Swift 6.0 |
| when `--assume-filename` beside `-` was first accepted | the guard in `Sources/swift-format/Subcommands/LintFormatOptions.swift` on `release/6.0`, `release/6.1`, `release/6.2` and `main` | Swift 6.2 |
| which Xcode ships Swift 6.2 | `https://www.swift.org/api/v1/install/releases.json` | Xcode 26, released 2025-09-15 |

Rows 1 to 4 are each one `curl` against the `swiftlang/swift-format` repository,
over the release branch of each Swift version. The `README.md` of `release/6.0`
states the answer to row 3 in its own words: "Xcode 16 and above include
swift-format in the toolchain. You can run `swift-format` from anywhere on the
system using `swift format` (notice the space instead of dash)." The `README.md`
of `release/5.10` carries no such line.

**Row 4 is the floor, and row 3 is not.** The run block reads each file on
standing input under `--assume-filename sah-probe.swift -`, because that is what
keeps the file NAME out of every line the tool writes. `release/6.0` and
`release/6.1` each guard the option with
`if assumeFilename != nil && !paths.isEmpty`, and `-` IS a path, so each raises
`ValidationError("'--assume-filename' is only valid when reading from stdin")`
and exits **64**. `release/6.2` and `main` each read
`!(paths.isEmpty || paths == ["-"])` instead, which lets the lone `-` through.
The guard at the foot of the file loop reads 64 as trouble, so under Swift 6.0
or Swift 6.1 EVERY file is declined with
`sah-diagnostic: idioms-swift declined <path>: swift format exited 64`, and the
gate judges nothing.

The guard shape was measured on Apple Swift 6.4 by giving the tool a NAMED path
beside the option: `swift format lint --strict --configuration rules.json
--assume-filename sah-probe.swift f.swift` writes
`Error: '--assume-filename' is only valid when reading from stdin` and exits 64.
The `release/6.0` and `release/6.1` sources were read rather than run, because
neither toolchain was available on this machine.

**To drop `-` does not lower the floor.** `release/6.0` `Frontend.swift` reads
standing input only for an EMPTY path list and carries no `-` branch at all,
while `release/6.1` and above write a ten-line deprecation warning for an empty
path list, which opens `<unknown>: error: Running swift-format without input
paths is deprecated`. The reading refuses each of those ten lines as trouble, so
that form declines every file from Swift 6.1 upward. No ONE invocation spans
Swift 6.0 to Swift 6.4, so the gate names 6.2 and asks for it.

`doctor.check_command` measures row 4 rather than row 3, because the check must
ask the toolchain for everything the run block asks it for. The check ends in
`printf '' | swift format lint --strict --configuration '{}' --assume-filename
sah-probe.swift -`, which gives the tool the same three options the run block
gives it and was measured to exit 0 on Apple Swift 6.4. A check that left
`--assume-filename` out would exit 0 at Swift 6.1 as well, and the rule would
then report HEALTHY over a gate that declines every file.
`the_shipped_swift_idioms_rule_doctor_measures_every_option_its_script_gives_the_tool`
holds the two option sets equal in BOTH directions, so an option added to the
run block and not to the check fails by name.

`swift format --version` writes `main` and names no release, so
`doctor.check_version_command` reads `swift --version`, which is the only
version a person can compare with the floor.

The rule declares no install commands, and `doctor.fix_hint` names the toolchain
rather than a package. Homebrew does not install the Swift toolchain: Xcode 26
and above ship it, and it is not a formula this gate can name.
`the_shipped_swift_idioms_rule_states_one_swift_version_floor` holds the floor
this section states against the floor the hint states, and holds the Xcode
release the same way, so the two cannot disagree.

**No rule of the allowlist reads a Swift LANGUAGE version.**
`swift format lint --help` names no option that states one, and the run states
none, so the toolchain floor above is the whole of what a version decides here.

`AlwaysUseLiteralForEmptyCollectionInit` writes
`replace '[Int]()' with ': [Int] = []'`, and
`replace '[String: Int]()' with ': [String: Int] = [:]'`. It stays SILENT for
`Set<String>()`, so the empty `Set` literal has no owner here and the
`idioms.md` bullet for it keeps it.

**The CI runner runs the same toolchain.** The self-hosted macOS runner writes
`swift --version` in the step "Ensure the language toolchains the roster tests
need are available". Read from run `34135649101`, at `2026-09-07T14:57:21Z`:

    Apple Swift version 6.4 (swiftlang-6.4.0.33.1 clang-2100.3.33.1)
    Target: arm64-apple-macosx27.0.0
    swift-driver version: 1.168.6

6.4 stands over the floor of 6.2, so the gate does not turn CI red for want of a
toolchain, and the `--assume-filename` pair the run block gives the tool is
accepted there.

## Why this rule supersedes nothing

`supersedes` names a whole prompt rule, and the engine skips that rule whole
when the tool is healthy. This gate decides BULLETS of
`builtin/validators/swift/rules/idioms.md` and not the whole of it, so naming
that rule here would take its OTHER bullets out of every review the moment the
toolchain is present.

`the_shipped_swift_idioms_tool_rule_owns_each_bullet_it_took` holds seven
bullets to being reported by this rule, over Swift written in the SHAPE the
deleted bullet named. The seven are the shorthand type sugar bullet, both halves
of the `Void` return clause, the empty `Array` and `Dictionary` literal, the
internal memberwise initializer identical to the synthesized one, the
`forEach` walk, and the `let` on each bound case variable.

`idioms.md` stays, and states only the bullets this gate cannot read: the empty
`Set` literal, the type-name repetition in every shape but one, and the `public
init` that repeats the synthesized initializer on a type that is not `public`.
A requirement has one owner, so none of those three is a row of the test above. `swift_judgment_rules.rs` holds the gate
silent over each of them instead.

`value-semantics.md` states "Mark a `class` that is not designed for subclassing
`final`" again. The toolchain carries no rule like `preferFinalClasses`, so the
prompt rule is the only owner, and the same file holds the gate silent over it.

`UseWhereClausesInForLoops` stays OFF, and no prompt rule states a `where`
clause.

`stuttering-name-go` and `unused-dependencies-rust` are the two shipped tool
rules that already declare no `supersedes`, so an empty key is the stated shape
rather than an omission.

## What the fixture pair holds

The failing fixture holds one declaration for each rule of the allowlist, each
written in the form its rule reports, under a `// MARK:` heading that names the
rule. Measured with the shipped script: **11 findings**, exit 0, carrying all
**7** rules. Three rules report more than one line, because the fixture holds
more than one declaration for each of them: two empty-collection calls, three
long type names and two return clauses.

The passing fixture holds the SAME declarations, each written in the form its
rule asks for. Measured with the shipped script: 0 findings, exit 0.

Neither fixture is clean for the pretty-printer, and that is deliberate. Both
are written with 4-space indentation, and the raw run over the passing fixture
writes 34 `[Indentation]` lines. The filter drops every one of them, so the
doctor's own fixture pair measures the filter as well as the rules.

The doctor materializes the fixture directory flat and hands this rule one
path, so neither fixture reads the other.
