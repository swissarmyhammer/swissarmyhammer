---
name: dead-code-swift
description: Swift declarations nothing in the module references — checked by periphery, not by prompt.
match:
  files:
    - "**/*.swift"
  project_types:
    - swift
supersedes: dead-code
tool:
  scope: workspace
  run: |
    if [ ! -f Package.swift ]; then
      echo "no Package.swift at the workspace root: periphery scans a built SPM package" >&2
      exit 1
    fi
    swift build --build-tests >&2 || exit 1
    store=""
    for candidate in .build/debug/index/store .build/out; do
      if [ -d "$candidate/v5" ]; then
        store="$candidate"
        break
      fi
    done
    if [ -z "$store" ]; then
      echo "swift build wrote no index store under .build: periphery has nothing to read" >&2
      exit 1
    fi
    manifest=$(swift package describe --type json)
    if [ -z "$manifest" ]; then
      echo "dead-code-swift: swift package describe wrote no manifest, so the run cannot tell a test target from a product target" >&2
      exit 1
    fi
    if ! test_paths=$(printf '%s\n' "$manifest" | jq -r '.targets[] | select(.type == "test") | .path'); then
      echo "dead-code-swift: jq could not read the package manifest, so the run cannot tell a test target from a product target" >&2
      exit 1
    fi
    report_filter=()
    while IFS= read -r test_path; do
      if [ -n "$test_path" ]; then
        report_filter+=(--report-exclude "$test_path/**")
      fi
    done <<< "$test_paths"
    periphery scan --quiet --format json --skip-build --index-store-path "$store" \
      --retain-public --retain-objc-accessible --retain-swift-ui-previews \
      --retain-codable-properties --disable-update-check --relative-results \
      "${report_filter[@]}" |
      jq -c '.[]
             | select(.kind != "var.parameter")
             | (.location | split(":")) as $at
             | {file: $at[0], line: ($at[1] | tonumber), message: "\(.kind) `\(.name)` is \(.hints | join(", "))"}'
  doctor:
    check_command: "which periphery swift jq && test -f Package.swift"
    check_version_command: "periphery version"
    fix_hint: "brew install periphery, and run the review from the directory holding Package.swift"
---

# Dead Code — Swift

`periphery` reports every Swift declaration the compiler's index store shows
nothing referencing. It is the only Swift tool that answers the question, and
the index store is what makes its answer a fact rather than a guess: the
declarations are compiled first, and periphery reads the reference graph the
compiler wrote.

## When this rule runs, and when the prompt rule runs instead

Periphery needs a built SPM package. The `doctor.check_command` therefore asks
two things — is the tool installed, and does the workspace root hold a
`Package.swift` — and a Swift project that answers no to either is reported by
`sah doctor` as missing, which leaves the `dead-code` prompt rule running for
its files. That is the designed fallback, not a gap. An Xcode-only project is
the common case: periphery can scan one, but only with a `--project`, a
`--schemes` and a `--targets` list that no rule can guess.

## The staging contract

Write `// periphery:ignore` on the line above a declaration a later change will
reference. Nothing else counts. A staged declaration with no marker is dead.

**The marker takes no trailing text.** Measured against periphery 3.8.0 on a
four-declaration probe: `// periphery:ignore` alone suppresses, and
`// periphery:ignore` on the line under a doc comment suppresses, but
`// periphery:ignore — the caller lands next` and
`// periphery:ignore  the caller lands next` both still report. Put the reason
on its own comment line above the marker.

`// periphery:ignore:all` at the top of a file covers a whole file, and
`// periphery:ignore:parameters a, b` covers named parameters.

## `--retain-public` is the exported-surface exemption

`--retain-public` is what makes this rule's claim the same claim the other four
languages make: an *internal* declaration nothing in the module references. A
`public` or `open` declaration is the module's surface for callers outside the
package, exactly as a `pub` item is in Rust, an exported identifier is in Go, and
a name without a leading underscore is in Dart.

The measurement says how much that one flag is worth. Over `Alamofire` at HEAD,
built with `swift build --build-tests`:

| Run | Findings |
|---|---|
| no retain flags | 493 |
| `--retain-public` and the rest of the flag set | 103 |
| the same, minus `var.parameter` | 74 |

The 493 are almost entirely Alamofire's public API, which is what a library
exists to expose.

`--retain-objc-accessible`, `--retain-swift-ui-previews` and
`--retain-codable-properties` are the framework-invoked exemptions: a declaration
the Objective-C runtime reaches by selector, a `#Preview` Xcode renders, and a
property an encoder reads by reflection all have callers no index can see.

## `--build-tests`, and why it changes the answer

`swift build` alone does not compile the test targets, so every internal helper
that only a test uses looks dead. Measured on Alamofire: `RequestTaskMap.isEmpty`
is reported without `--build-tests` and not reported with it, because a test is
its only caller.

## The test targets are indexed, and never reported

`--build-tests` also brings the test-support code itself under the gate, and
`dead-code`, the prompt rule this one supersedes, exempts "test functions and
test-only helpers". So the run has to hold two things at once: the test targets
stay in the INDEX, where they count as callers, and they stay out of the REPORT.

`--report-exclude` is that split, and periphery states it in those words —
"Source file globs to exclude from the results. Note that this option is purely
cosmetic, these files will still be indexed."

The script asks `swift package describe --type json` which targets are of type
`test` and writes one `--report-exclude <path>/**` for each. The manifest is
where the paths come from, so the split reads a fact of the package rather than
the `Tests/` naming convention: Alamofire declares one test target at `Tests`,
swift-nio declares fifteen at `Tests/<Name>`.

Measured over `Alamofire` at `0455bfb` with periphery 3.8.0, built with
`swift build --build-tests`, each run minus `var.parameter`:

| Run | Findings | Where they stand |
|---|---|---|
| no report filter | 74 | 22 in `Source/`, 52 in `Tests/` |
| `--report-exclude Tests/**` | 22 | every one in `Source/` |
| `--exclude-tests` instead | 25 | every one in `Source/` |

The 52 are test-only helpers word for word: 30 of them stand in one file of
`AFError` convenience properties no test ever calls. The 22 the filter keeps are
the same 22 the unfiltered run reported in `Source/`, declaration for
declaration, so the filter drops findings and changes no analysis.

`--exclude-tests` answers another question. It takes the test targets out of the
INDEX, so the tests stop being callers, and the run then reports three
declarations a test does call: `RequestTaskMap.isEmpty`,
`OfflineRetrier.init(monitor:maximumWait:isOfflineError:)` and
`RequestInterceptor.retryRequired`. Those three are the whole reason the run
builds the tests, so the flag that reads like the shorter spelling of this
carve-out is the one flag that breaks it.

Of the 22 findings in `Source/`, the ones hand-checked are real:
`Protected.around(_:)`, `Protected.withState(perform:)` and
`Protected.attemptToTransitionTo(_:)` are internal methods with no caller;
`RequestTaskMap.count`, `.eventCount`, `.requests` and `.isEventsEmpty` are
internal computed properties nothing reads; and the three
`typealias Failure = Downstream.Failure` declarations inside a `private final
class Inner: Subscription` are redundant, because `Subscription` requires no
`Failure`.

## `assignOnlyProperty` and the reads periphery cannot see

Periphery hints `assignOnlyProperty` on a property it sees written and never
read. The index is right about what it can see, and it cannot see the `==` and
`hash(into:)` the compiler *synthesizes* for an `Equatable` or `Hashable` type
that writes neither. Those bodies read every stored property, so a type that
declares the conformance and compares whole values has readers for all of them —
and periphery reports them all as assign-only.

Measured on `FoundationModelsRouter` at `fe0a645`, periphery 3.8.0: eight
findings, all `var.instance … is assignOnlyProperty`, across a six-property
`Equatable` struct and a two-property `Hashable` struct. The only reads are the
synthesized ones, exercised by three lines that compare whole values.

**Do not delete such a property.** It does not compile — the memberwise
initializer and every literal still supply it — and deleting all of them makes
`a == b` true for any two values, so a test that compares them silently asserts
nothing while still passing.

Write `// periphery:ignore` above the property, with the reason on its own line
above the marker, exactly as the staging contract requires:

```swift
/// Read by the synthesized `Equatable` conformance; periphery sees no caller.
// periphery:ignore
let deliveredToolOutputs: [String]
```

The blanket `--retain-assign-only-properties` flag is deliberately **not** set.
It would also retain a property nothing reads at all — the `deadCount` case this
rule's own failing fixture exists to catch — and that is real dead code worth
reporting. The distinction is whether a reader exists; the marker is how a human
states that one does.

## Why `var.parameter` is dropped

Periphery reports an unused function *parameter*. That is not dead code: the
parameter is part of a signature every caller must supply, so nothing about it
can be deleted without changing the call sites. Measured on Alamofire, 29 of the
103 findings are `var.parameter`, and the ones read by hand are all the Swift
phantom-type idiom — `func responseDecodable<Value>(of type: Value.Type =
Value.self, ...)`, where the parameter exists only to pin a generic.

Dropping the kind also keeps Swift in line with the other four languages: rustc
`dead_code`, staticcheck `U1000`, `dart analyze`'s four unused diagnostics, and
`ts-prune` all leave parameters alone. This is attribution, not exemption — to
exempt one parameter, write `// periphery:ignore:parameters <name>` in the code.

## How the run is shaped

The scope is `workspace` because periphery reads a whole package's index.
The engine keeps only the findings in the changed files.

`periphery scan` on its own fails on Swift 6.4: it runs `swift build` and then
looks for the index store at `.build/debug/index/store`, which the SwiftPM of
that release no longer writes — the store is `.build/out`, holding the same
`v5` index format directory. The script therefore builds first and finds the
store itself, checking the old location before the new one so both SwiftPM
layouts work. Without the store there is nothing to judge, so the script exits
nonzero and says so on stderr rather than reporting a clean run.

`--relative-results` makes periphery print `Source/Core/AFError.swift:619:9`
rather than an absolute path, so the `jq` only has to split the column off.

The rule declares no install commands. Homebrew is the supported way to install
periphery and it installs the current version only, so a Homebrew command cannot
pin one — the same reason `function-length-swift` and `missing-docs-swift` state.
The `doctor.fix_hint` names the Homebrew command instead. `sah doctor` shows
that hint as the fix; the install lifecycle never runs it.
