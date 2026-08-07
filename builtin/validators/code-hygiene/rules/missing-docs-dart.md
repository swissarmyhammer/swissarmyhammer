---
name: missing-docs-dart
description: Public Dart members need doc comments — checked by dart analyze, not by prompt.
match:
  files:
    - "**/*.dart"
  project_types:
    - flutter
supersedes: missing-docs
tool:
  scope: files
  run: |
    package="$(cd "$(mktemp -d)" && pwd -P)"
    printf '%s\n' 'name: sah_missing_docs_probe' 'environment:' "  sdk: '>=3.0.0 <5.0.0'" > "$package/pubspec.yaml"
    printf '%s\n' 'linter:' '  rules:' '    - public_member_api_docs' > "$package/analysis_options.yaml"
    for file in "$@"; do
      copy="$package/lib/${file#/}"
      mkdir -p "$(dirname "$copy")"
      cp "$file" "$copy"
    done
    (cd "$package" && dart pub get --offline) > /dev/null 2>&1
    dart analyze --format=machine "$package" |
      awk -F'|' -v prefix="$package/lib/" '
        $3 == "PUBLIC_MEMBER_API_DOCS" && index($4, prefix) == 1 {
          printf "%s:%s: %s\n", substr($4, length(prefix) + 1), $5, $8
        }'
  doctor:
    check_command: "which dart awk"
    check_version_command: "dart --version"
    fix_hint: "brew install dart-sdk"
---

# Missing Documentation — Dart

`dart analyze` reports every public member without a doc comment when the
`public_member_api_docs` lint is on. The lint is opt-in.

`dart analyze` takes no rule flag. It reads `analysis_options.yaml` by walking
up the directory tree from each analyzed file, so the only way for the rule to
own its configuration is to build the tree the analyzer walks. The script makes
a probe package in a temporary directory, copies the changed files under its
`lib/`, analyzes the package, and maps the temporary paths back to the paths it
was given. The project's own `analysis_options.yaml` is never read.

Two properties of the lint make the probe package necessary, and both fail
silently rather than loudly:

- `public_member_api_docs` reports only for a file inside a package's `lib/`
  directory. A loose file with the configuration beside it reports nothing.
- The analyzer needs `.dart_tool/package_config.json` to recognize the package,
  and only `dart pub get` writes it. Without it this lint stays quiet while
  other lints still report. `--offline` keeps the probe package, which declares
  no dependencies, off the network.

The temporary directory is resolved with `pwd -P` before use. On macOS
`mktemp -d` returns a path through a symbolic link (`/var/...`) while `dart
analyze` reports the resolved path (`/private/var/...`), and the prefix strip
would match nothing.

The pipe ends in `awk` rather than `grep` because `grep` exits nonzero when it
matches nothing, which the engine reads as a broken tool on every clean run.

The scope is `files` because the probe package holds the files the script is
given.

The rule declares no install commands. `dart analyze` is a component of the
Dart SDK, not a package with its own version, so no install command can pin it.
The `doctor.fix_hint` states `brew install dart-sdk` instead. `sah doctor` shows
that hint as the fix; the install lifecycle never runs it.

Selection in the pipe is attribution, not exemption: to exempt one member,
write `// ignore: public_member_api_docs` above it in the code.
