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
    config="$(mktemp -d)/swiftlint.yml"
    printf '%s\n' 'only_rules:' '  - no_magic_numbers' \
      'no_magic_numbers:' '  allowed_numbers: [0, 1, -1, 100]' > "$config"
    swiftlint lint --config "$config" --no-cache --quiet --reporter json "$@" |
      jq -c '.[] | select(.rule_id == "no_magic_numbers")
             | {file: .file, line: .line, message: .reason}'
  doctor:
    check_command: "which swiftlint jq"
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
config states `[0, 1, -1, 100]` and the two lists then agree.

The script writes its own `swiftlint.yml` to a temporary path and passes it with
`--config`. `only_rules` turns the `no_magic_numbers` rule on and every other
swiftlint rule off, so the rule owns its whole invocation and never reads the
project's own `.swiftlint.yml`. `--no-cache` keeps swiftlint from writing a cache
directory into the workspace.

The scope is `files` because swiftlint reads the paths it is given.

The rule declares no install commands. Homebrew is the supported way to install
swiftlint, and it installs the current version only, so a Homebrew command
cannot pin one. Mint can pin one — `mint install realm/SwiftLint@0.65.0` — but
it builds swiftlint from source and links the result into `~/.mint/bin`, which
is not on the path, so the command cannot make `check_command` pass. The
`doctor.fix_hint` states `brew install swiftlint` instead. `sah doctor` shows
that hint as the fix; the install lifecycle never runs it.

Selection in the pipe is attribution, not exemption: to exempt one literal, write
`// swiftlint:disable:next no_magic_numbers` above it in the code.
