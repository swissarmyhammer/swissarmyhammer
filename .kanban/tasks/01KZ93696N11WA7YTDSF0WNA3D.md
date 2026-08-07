---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kze80hnm8wg4mmjepm5af841
  text: |-
    Research done. Every pipeline below was run in a real terminal on this machine (macOS arm64) before the frontmatter was written.

    **TypeScript/JavaScript — eslint + eslint-plugin-jsdoc.** Installed `eslint@10.8.0 eslint-plugin-jsdoc@63.3.3 typescript-eslint@8.66.0 typescript@5.9.3` with `npm install -g`. The generated flat config is a `.cjs` file in a temp dir. A temp config cannot resolve a plugin by name, because node walks up from the config file and finds no `node_modules`. `NODE_PATH="$(npm root -g)"` makes `require()` resolve the global packages. `--no-config-lookup` keeps eslint off the project's own config. `typescript-eslint`'s parser is needed for `.ts`; the default parser cannot read TypeScript syntax. `typescript-eslint@8.66.0` accepts `typescript >=4.8.4 <6.1.0`, so the pin is 5.9.3, not the 7.0.2 that is `latest`.

    **Go — revive `exported`.** Installed `go@1.26.5` (brew) and `go install github.com/mgechev/revive@v1.15.0`. The generated config is a one-line `revive.toml` holding `[rule.exported]`. `-formatter json` prints `null`, not `[]`, on a clean file, so the jq filter starts `(. // [])[]`. revive needs no `go.mod` to lint a loose file.

    **Swift — swiftlint `missing_docs`.** Installed `swiftlint@0.65.0` (brew). The generated config is a `swiftlint.yml` with `only_rules: [missing_docs]`. `--no-cache` keeps swiftlint from writing a cache beside the code.

    **Dart — `public_member_api_docs`.** This is the generated-config case the card called out. `dart analyze` reads `analysis_options.yaml` by walking up from the file, and takes no rule flag. Two more constraints found by running it:
    1. `public_member_api_docs` only fires for a file inside a package's `lib/`. A loose file in a temp dir with the config beside it reports nothing.
    2. It also needs `.dart_tool/package_config.json`, which only `dart pub get` writes. Without it the lint is silent while other lints (`prefer_single_quotes`) still fire — a silent false pass.
    So the script builds a whole probe package in a temp dir (`pubspec.yaml`, `analysis_options.yaml`, the changed files copied under `lib/`), runs `dart pub get --offline`, analyzes the package, then maps the temp paths back.
    A third trap: `mktemp -d` returns `/var/folders/...` while `dart analyze` reports the resolved `/private/var/folders/...`. The prefix strip silently matched nothing. The script resolves the temp dir with `cd ... && pwd -P` first.
    The pipe ends in `awk`, not `grep`: `grep` exits 1 when it matches nothing, which the engine reads as a broken tool on every clean run.
  timestamp: 2026-08-07T13:53:42.068920+00:00
- actor: claude-code
  id: 01kze8tb1nk9v4g8ztp6nvc32x
  text: |-
    All four pipelines ran on this machine. None was written from memory.

    Every pipeline was then run again the way the doctor runs it — working directory `builtin/validators/code-hygiene/fixtures/`, the fixture file name as the only argument, findings counted only when the reported path names that fixture. All six shipped rules pass their pair:

        PASS missing-docs-rust:       fail=1 pass=0
        PASS missing-docs-python:     fail=1 pass=0
        PASS missing-docs-typescript: fail=1 pass=0
        PASS missing-docs-go:         fail=1 pass=0
        PASS missing-docs-swift:      fail=1 pass=0
        PASS missing-docs-dart:       fail=1 pass=0

    **Install commands, and why two rules declare none.**

    - TypeScript: `npm install -g eslint@10.8.0 eslint-plugin-jsdoc@63.3.3 typescript-eslint@8.66.0 typescript@5.9.3`. Run and verified.
    - Go: `go install github.com/mgechev/revive@v1.15.0`. Run and verified. It puts the binary in `$(go env GOPATH)/bin`, so a machine without that directory on its path reports the rule degraded until the person adds it.
    - Swift: no install commands, `fix_hint: "brew install swiftlint"`. This is the ^s297bfh mechanism. Homebrew installs the current version only and cannot pin. Mint can pin, and `mint install realm/SwiftLint@0.65.0` was run here and did produce SwiftLint 0.65.0 — but it builds from source and links into `~/.mint/bin`, which is not on the path, so the command cannot make `check_command` pass. A command that cannot satisfy its own doctor check is not an install command.
    - Dart: no install commands, `fix_hint: "brew install dart-sdk"`. `dart analyze` is a component of the Dart SDK, the same shape as clippy in a rustup toolchain, so there is no package version to pin.

    **The `readlink` step in the eslint pipeline.** The first draft set `NODE_PATH="$(npm root -g)"`. It reported zero findings on the fail fixture. Two `npm` commands were on the path, and `npm root -g` answered for the first one while eslint came from the other, so `require("eslint-plugin-jsdoc")` failed inside the temp config. Because the pipe ends in `jq`, the broken run looked exactly like a clean file. The script now reads the module tree from the eslint command itself (`readlink -f` on the command, then two directories up), which also names the right tree for a project-local eslint.

    **Test change.** `every_shipped_missing_docs_tool_rule_passes_its_fixtures` now covers all six rules. It asserted `usable()` for every rule; with four more toolchains in the roster that assertion states what the machine has installed, not what the code guarantees, and the doctor contract says a missing tool is a warning that falls the rule back to its prompt rule. The test now asserts `supersedes` for every rule, asserts the fixtures passed for every rule whose tool doctor found, and asserts at least one rule was exercised so it can never pass while asserting nothing. With the tools installed here, all six take the fixture branch.
  timestamp: 2026-08-07T14:07:47.253607+00:00
- actor: claude-code
  id: 01kze8thrhspr4kmevcszzvvbr
  text: |-
    ### implement — changed
    - evidence: 13 files — 4 rule files and 8 fixtures under builtin/validators/code-hygiene/, builtin/validators/README.md, crates/mirdan/src/builtin_validators.rs, crates/swissarmyhammer-validators/src/builtin/mod.rs, crates/swissarmyhammer-validators/src/review/tool_rules.rs. cargo test -p swissarmyhammer-validators -p mirdan: 937 passed, 0 failed. cargo clippy on both crates: no warnings.
    - next: /review
  timestamp: 2026-08-07T14:07:54.129350+00:00
- actor: claude-code
  id: 01kze9xwf1p5cd49wc3pa1275n
  text: |
    ### review — findings
    - evidence: 3 findings (3 confirmed, 1 refuted, 12 attempted) — builtin/validators/code-hygiene/fixtures/missing-docs-go.fail.go:12, builtin/validators/code-hygiene/fixtures/missing-docs-swift.fail.swift:10, builtin/validators/code-hygiene/fixtures/missing-docs-typescript.fail.ts:11
    - scope: review sha HEAD~1..HEAD (e2e1e2d49)
    - deviations judged: the Swift and Dart rules declare doctor.fix_hint and no install.commands, which matches the mechanism the Rust rule already uses; the Dart script builds a temporary package, and the validators README documents that case. The engine raised no finding against either.
    - next: add undocumented types, structures, classes, interfaces, enums, and methods to the three fail fixtures, so each fail fixture covers every category its pass fixture covers.
  timestamp: 2026-08-07T14:27:11.969428+00:00
- actor: claude-code
  id: 01kze9yqr8bfy4088dbdjm4gw8
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 13 files; four tool rules with eight fixtures. Every pipeline was executed in a terminal first. Two deviations accepted by the review: Swift and Dart declare a doctor.fix_hint in place of install commands, and the Dart script builds a probe package in a temp directory.
    - test: green — cargo nextest run --workspace 13761 passed, doc tests 0 failed, fmt clean, clippy clean. The non-Rust fixtures disturb no other build.
    - commit: e2e1e2d49
    - review: findings — builtin/validators/code-hygiene/fixtures/missing-docs-go.fail.go:12, missing-docs-swift.fail.swift:10, missing-docs-typescript.fail.ts:11 (each fail fixture exercises only a function while its pass fixture covers many kinds)
  timestamp: 2026-08-07T14:27:39.912062+00:00
- actor: claude-code
  id: 01kzeaa4xy51a6br8pwh5vmtwf
  text: |-
    The three findings name one cause: a fail fixture that is narrower than its pass fixture. The cause was removed from every fail fixture of the family, not only from the three the review named.

    Each fail fixture now holds one undocumented item of every kind its pass fixture documents:

    - Go: an exported type, a method on it, and a function.
    - Swift: a public structure, a public method in it, and a public function.
    - TypeScript: an interface, a type alias, an enumeration, a class, a method, and a function.
    - Dart: a class, a method in it, and a function.
    - Python: a class, a method in it, and a function.
    - Rust: a struct and a function.

    Every pipeline was run again the way the doctor runs it — working directory `builtin/validators/code-hygiene/fixtures/`, the fixture name as the only argument for a `files`-scope rule, no argument for the `workspace`-scope Rust rule, and findings counted only when the reported path names the fixture under test:

        PASS missing-docs-dart:       fail=3 pass=0
        PASS missing-docs-go:         fail=3 pass=0
        PASS missing-docs-python:     fail=3 pass=0
        PASS missing-docs-rust:       fail=2 pass=0
        PASS missing-docs-swift:      fail=3 pass=0
        PASS missing-docs-typescript: fail=6 pass=0

    The raw tool output was read for each fail fixture to prove the count is one finding for each kind, not many findings about one item. revive names the type, the method, and the function on their own lines; ruff reports D101, D102, and D103; clippy reports "missing documentation for a struct" and "missing documentation for a function"; eslint reports the six exported items at their own lines.

    The written contract now states the rule, so the next tool rule does not repeat the defect. `builtin/validators/README.md` says in the Fixtures section that the two fixtures must cover the same kinds, and that a pass fixture which documents six kinds against a fail fixture which holds only a function proves nothing about the other five.

    `revive` is not on the default path — it lives in `$(go env GOPATH)/bin`. The manual runs and the shipped test both need that directory added, or the Go rule reports its tool missing and is skipped.

    `cargo test -p mirdan --lib` failed two times in one run on `install::tests::test_deploy_plugin_creates_files` and `install::tests::test_deploy_plugin_twice_overwrites_cleanly`, with `cannot read MIRDAN_AGENTS_CONFIG '/nonexistent/agents.yaml'`. Both pass alone, and the full lib target passes again with 432 tests. It is environment-variable cross-talk between tests in one process, and no file in mirdan was changed by this work.
  timestamp: 2026-08-07T14:33:53.854195+00:00
- actor: claude-code
  id: 01kzeaab13rs4bykt10mw2kxpm
  text: |-
    ### implement — changed
    - evidence: 8 files — builtin/validators/README.md, builtin/validators/code-hygiene/fixtures/lib.rs, and the six fail fixtures (missing-docs-{rust,python,typescript,go,swift,dart}.fail.*). Doctor-shape re-run of all six pairs: fail>=1 and pass=0 for each. cargo test -p swissarmyhammer-validators: 503 passed, 0 failed, doc tests 2 passed; every_shipped_missing_docs_tool_rule_passes_its_fixtures ok. cargo test -p mirdan --lib: 432 passed, 0 failed.
    - next: /review
  timestamp: 2026-08-07T14:34:00.099257+00:00
depends_on:
- 01KZ9361Q5W4W38TGRWB01GTZG
- 01KZ935S9GWN207TF50MHCN5HB
position_column: doing
position_ordinal: '8480'
title: missing-docs runners for TS, Swift, Go, and Dart
---
Clone the missing-docs tool-rule pattern to the remaining languages. Each is a rule file in `code-hygiene/rules/` with a `tool` block and `supersedes: missing-docs`.

- TypeScript/JavaScript: eslint with `jsdoc/require-jsdoc`. Generate a flat config in a temp path; pass with `--config`. JSON via `--format json`.
- Swift: swiftlint rule `missing_docs` (opt-in). Generate a config in a temp path. JSON via `--reporter json`.
- Go: revive rule `exported`. Generate the toml config in a temp path.
- Dart: `public_member_api_docs`. `dart analyze` takes no per-run rule flags — this is the generated-config test case. Solve it last.

Each tool rule ships fail/pass fixtures and pinned install commands. Follow the pattern proven by the Rust and Python tool rules (^b01gtzg).

#tool-validators

## Review Findings (2026-08-07 09:20)

- [x] `builtin/validators/code-hygiene/fixtures/missing-docs-go.fail.go:12` — The pass fixture tests documented types and methods, but the fail fixture only tests an undocumented function. If the tool fails to report undocumented types or methods, the fixture will not catch it. Add an undocumented type and method to the fail fixture, e.g., `type UndocumentedType struct{}` and `func (u UndocumentedType) UndocumentedMethod() {}`, so the fixture comprehensively proves the tool reports undocumented items across all categories.
- [x] `builtin/validators/code-hygiene/fixtures/missing-docs-swift.fail.swift:10` — The pass fixture tests documented structures and methods, but the fail fixture only tests an undocumented function. If the tool fails to report undocumented structures or methods, the fixture will not catch it. Add an undocumented structure and method to the fail fixture, e.g., `public struct UndocumentedStructure { public func undocumentedMethod() {} }`, so the fixture comprehensively proves the tool reports undocumented items across all categories.
- [x] `builtin/validators/code-hygiene/fixtures/missing-docs-typescript.fail.ts:11` — The pass fixture tests documented interfaces, types, enums, classes, and methods, but the fail fixture only tests an undocumented function. If the tool fails to report undocumented interfaces, enums, classes, or methods, the fixture will not catch it. Add undocumented versions of the interfaces, types, enums, classes, and methods to the fail fixture, e.g., `export interface UndocumentedInterface {}`, `export class UndocumentedClass { undocumentedMethod(): void {} }`, so the fixture comprehensively proves the tool reports undocumented items across all categories.
