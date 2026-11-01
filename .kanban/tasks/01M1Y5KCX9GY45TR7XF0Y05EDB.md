---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m6bzsqr09racf0ntcefy0pe3
  text: |-
    ### implement — changed
    - evidence: 1 file, .github/workflows/ci.yml. Removed the `swiftformat` install block. Added step "Require a Swift toolchain that can run swift format" after the toolchain step: fails with a clear ::error:: when Swift is below 6.2 or the version cannot be read. Floor 6.2 comes from the measurement task; runner measured Swift 6.4 (run 34135649101), local 6.4.2. Checks: `rg swiftformat .github/` finds nothing; `actionlint .github/workflows/ci.yml` exit 0; guard run with stub versions 6.4.2 and 6.2 pass, 6.1.2, 5.10 and empty fail with the message. Swiftlint and periphery installs kept. No CI doc names the install. No text-assert tests written. Not run here: the cargo swift test set, `sah doctor` on the runner, and the CI push (no commit or push allowed).
    - next: review; confirm macOS job green after push.
  timestamp: 2026-11-01T16:41:34.208196+00:00
- actor: claude-code
  id: 01m6bztrnwaae3c5qs26y26qgf
  text: |-
    ### commit — changed
    - evidence: cc8c452af ci: stop installing swiftformat and require Swift 6.2 for swift format (.github/workflows/ci.yml)
    - next: review
  timestamp: 2026-11-01T16:42:07.932626+00:00
- actor: claude-code
  id: 01m6bzwre07gzb91f5dav1nx68
  text: |-
    ### review — clean
    - evidence: review sha cc8c452af~1..cc8c452af: 1 file attempted, 0 findings, 0 refuted, 0 failed. Hand-measured against the card: `rg -i swiftformat .github` finds nothing; swiftlint and periphery installs remain in ci.yml (swiftlint install, periphery install with the brew uninstall --cask line); `actionlint .github/workflows/ci.yml` exit 0; guard re-extracted from ci.yml and run against stub swift: 6.4.2, 6.2, 6.10, 7.0 exit 0; 6.1.2, 5.10 and empty version exit 1 with ::error:: messages; floor 6.2 matches the idioms-swift rule floor (builtin/validators/code-hygiene/rules/idioms-swift.md, fix_hint and floor section).
    - not measured: runner-side items (cargo nextest swift set, sah doctor on runner, CI green after push) need the runner and a push.
    - next: task moved to done.
  timestamp: 2026-11-01T16:43:13.216248+00:00
depends_on:
- 01M1Y5GD85944ACE2XS6SA5XWJ
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffffdc80
title: Stop installing swiftformat in CI
---
## What

`.github/workflows/ci.yml` installs `swiftformat` with Homebrew on the self-hosted macOS runner. No rule uses that tool after the task "Move idioms-swift to the toolchain swift format". Remove the install.

This task depends only on "Move idioms-swift to the toolchain swift format", because that is the last rule that runs swiftformat. `missing-docs-swift` runs swiftlint, and CI keeps installing swiftlint.

Files to change:
- `.github/workflows/ci.yml`

Remove only this block:

```
if ! command -v swiftformat &>/dev/null; then
  brew install swiftformat
fi
```

KEEP the `swiftlint` install, because `disallowed-constructs-swift`, `function-length-swift` and `magic-numbers-swift` still run it. KEEP the `periphery` install, with its `brew uninstall --cask` line, because `dead-code-swift` still runs it.

The comment above the install block reads "`swiftlint`, `periphery` and `dart` ship only through Homebrew". It names no swiftformat, thus it is already correct. Change it only if you find it wrong.

The `swift --version` step stays, and it becomes more important: the toolchain is now the formatter. Add a guard that makes the workflow fail with a clear message when the runner's Swift version is below the floor the measurement task states. Row 8 of that task holds the floor and the runner's own version.

Name a step by its name and a block by its text. Do not name a line number, because a line number moves.

## Acceptance Criteria
- [ ] `.github/workflows/ci.yml` names `swiftformat` nowhere.
- [ ] The workflow still installs `swiftlint` and `periphery`.
- [ ] The workflow fails with a clear message when the runner's Swift version is below the stated floor.
- [ ] CI passes on the self-hosted macOS runner, and every Swift tool rule reports as healthy.

## Tests
- [ ] Run `rg swiftformat .github/`. The command finds nothing.
- [ ] Run the full Swift test set on the runner: `cargo nextest run -p swissarmyhammer-validators swift`. Every test passes with no `swiftformat` on the PATH.
- [ ] Run `sah doctor` on the runner. Every Swift tool rule reports as healthy.
- [ ] Test the version guard: run the guard with a version string below the floor, and it fails with the stated message.
- [ ] Push the branch and read the CI result. The macOS job is green.

## Workflow
- Use `/tdd` — write the failing tests first, then write the code that makes them pass. #swift