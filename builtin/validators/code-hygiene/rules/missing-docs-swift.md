---
name: missing-docs-swift
description: Public Swift declarations need docs — checked by swiftlint, not by prompt.
match:
  files:
    - "**/*.swift"
  project_types:
    - swift
supersedes: missing-docs
tool:
  scope: files
  run: |
    config="$(mktemp -d)/swiftlint.yml"
    printf '%s\n' 'only_rules:' '  - missing_docs' > "$config"
    swiftlint lint --config "$config" --no-cache --quiet --reporter json "$@" |
      jq -c '.[] | select(.rule_id == "missing_docs")
             | {file: .file, line: .line, message: .reason}'
  doctor:
    check_command: "which swiftlint jq"
    check_version_command: "swiftlint version"
    fix_hint: "brew install swiftlint"
---

# Missing Documentation — Swift

`swiftlint` reports every `open` and `public` declaration without a doc
comment. The `missing_docs` rule names that check. It is opt-in, so it never
runs unless a configuration turns it on.

The script writes its own `swiftlint.yml` to a temporary path and passes it
with `--config`. `only_rules` turns the `missing_docs` rule on and every other
swiftlint rule off, so the rule owns its whole invocation and never reads the
project's own `.swiftlint.yml`. `--no-cache` keeps swiftlint from writing a
cache directory into the workspace.

The scope is `files` because swiftlint reads the paths it is given.

The rule declares no install commands. Homebrew is the supported way to install
swiftlint, and it installs the current version only, so a Homebrew command
cannot pin one. Mint can pin one — `mint install realm/SwiftLint@0.65.0` — but
it builds swiftlint from source and links the result into `~/.mint/bin`, which
is not on the path, so the command cannot make `check_command` pass. The
`doctor.fix_hint` states `brew install swiftlint` instead. `sah doctor` shows
that hint as the fix; the install lifecycle never runs it.

Selection in the pipe is attribution, not exemption: to exempt one declaration,
write `// swiftlint:disable:next missing_docs` above it in the code.
