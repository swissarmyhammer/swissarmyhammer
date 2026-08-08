---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzh5s7tr1q5qrbhnvkb94rvf
  text: |-
    Research done. Tools installed and measured before any rule was written.

    Installed: `staticcheck` 2025.1.1 (`go install honnef.co/go/tools/cmd/staticcheck@2025.1.1`), `ts-prune` 0.10.3 (`npm install -g ts-prune@0.10.3`). Already present: `vulture` 2.14, `periphery` 3.8.0, `dart` 3.11.0, `swift` 6.4, `go` 1.26.5.

    Measurements on real code:
    - Rust `cargo check --workspace --all-targets` `dead_code`: 0 findings on this workspace. Verified cargo REPLAYS cached warnings on a probe crate, so a second review run still reports; `--all-targets` doubles each finding, so `sort -u` is needed.
    - Rust orphan modules: 5 findings on this workspace, all hand-checked and all real — `crates/swissarmyhammer/src/security.rs` (15 KB nothing compiles), `crates/markdowndown/src/error.rs`, `crates/markdowndown/src/fetch.rs`, `crates/swissarmyhammer-common/src/sample_avp_test.rs`, `crates/swissarmyhammer-tools/src/mcp/notifications.rs` (0 bytes). 6.7 s over the whole workspace.
    - Python vulture at default confidence over requests + flask: 118 findings. `--ignore-names` drops 4 (dunder protocol methods, `test_*`). `--ignore-decorators` drops 14 (all `@t.overload` stubs). 100 remain, the mass of them library public API that neither library declares in `__all__`. Verified vulture honours `__all__` as the export marker, and that `# noqa: V103` / `V102` / `V201` suppress by code while a wrong code does not.
    - TypeScript ts-prune over `apps/kanban-app/ui`: 178 lines, 103 of them `(used in module)` (the symbol is alive, only the `export` keyword is surplus — not dead code), 18 of the rest in the generated `parser.terms.d.ts`. Verified `// ts-prune-ignore-next` suppresses.
    - Dart `dart analyze --format=machine` over dart-lang/http: 1 finding, real. Verified all four `// ignore:` markers suppress.
    - Swift periphery 3.8.0: `periphery scan` alone fails — Swift 6.4 SwiftPM writes the index store to `.build/out`, not the `.build/debug/index/store` periphery looks for. `swift build` then `periphery scan --skip-build --index-store-path` over the detected store works. Verified `// periphery:ignore` suppresses.

    Pre-existing defect found, out of scope for this card: the 5 orphan Rust modules above. They are not in this card's changed files, so the rule never reports them here.
  timestamp: 2026-08-08T17:12:28.760164+00:00
- actor: claude-code
  id: 01kzh7r8fe87dvefhn9q1q0k1s
  text: |-
    Implementation landed. Every column of the matrix row is covered.

    Six tool rules, each `supersedes: dead-code`:
    - `dead-code-rust` — `cargo check --workspace --all-targets --message-format=json` piped to jq on `dead_code`, plus the orphan-module scan the compiler cannot make (crate = nearest `Cargo.toml`; `lib.rs`/`main.rs`/`mod.rs`/`build.rs` and `tests/`, `benches/`, `examples/`, `src/bin/` exempt; `#[path]` targets exempt). Marker `#[expect(dead_code, reason = "...")]`, chosen over `#[allow]` because it expires through `unfulfilled_lint_expectations`.
    - `unused-code-go` — promoted, body rewritten to say why the compiler plus U1000 leaves nothing to judge. Marker `//lint:ignore U1000 <reason>`; `//nolint:staticcheck` measured as NOT working.
    - `dead-code-typescript` — ts-prune 0.10.3, discovers every `tsconfig.json` outside `node_modules`, drops `(used in module)` and `.d.ts`, `sort -u`.
    - `dead-code-python` — vulture at default confidence, `--ignore-names`/`--ignore-decorators`/`--exclude` in the script; `unreachable-code-python` deleted and folded in.
    - `dead-code-dart` — `dart analyze --format=machine .` selecting the four unused diagnostics, prefix stripped with `pwd -P`.
    - `dead-code-swift` — `swift build --build-tests` then `periphery scan --skip-build --index-store-path <detected>` with `--retain-public` and friends; `var.parameter` dropped; `check_command` also tests for `Package.swift` so a project without an SPM package falls back to the prompt rule.

    Docs: `dead-code.md` rewritten as the fallback with the annotation contract table; `builtin/validators/README.md` rules-for-tool-rules gained "an exemption a person would argue for in prose must become an inline suppression the tool reads", with staged work as the canonical example and a note to prefer a marker that expires; `code-hygiene/VALIDATOR.md` records the reversal of ^teemmch and marks the knip, periphery and vulture verdicts superseded.

    Rosters: `SHIPPED_DEAD_CODE_RULES` (tool_rules.rs), `CODE_HYGIENE_DEAD_CODE_TOOL_RULES` (builtin/mod.rs), and the fixture list (mirdan/builtin_validators.rs) all carry the six. The Python acceptance test was inverted — it now asserts the tool rule DOES suppress `dead-code`, and its probe module uses `__all__` so the run measures the one stranded statement.

    RED verified eight ways, each restored to GREEN:
    1-5. Strip the staging marker from each pass fixture (rust, python, typescript, dart, go) — doctor reports "the pass fixture ... produced 6 / 8 / 5 / 5 / 6 finding(s); none are allowed".
    6. Annotate every item in the Rust fail fixture — "the fail fixture produced no findings; at least one is required".
    7. The same for the Python fail fixture.
    8. Drop `supersedes: dead-code` from `unused-code-go` — "unused-code-go must supersede dead-code, the contract every dead-code tool rule keeps".

    Swift is measured outside the acceptance test on purpose: `check_command` requires a `Package.swift` at the process cwd, so doctor reports it Missing in this repo. Verified in a scratch SPM project carrying a project-layer `.validators/code-hygiene` copy — `sah doctor` printed `Validator Tool Rule · code-hygiene/dead-code-swift ┆ tool present (3.8.0); fixtures pass`, and the script reported the one real dead function in that project and left the `public` entry and the reached helper alone. Nothing under `~` was touched.

    Tests: swissarmyhammer-validators lib 526 pass, mirdan lib 501 pass, swissarmyhammer-cli doctor 17 pass, tools review integration 4 + 1 + 3 + 1 + 2 pass. `cargo clippy -p swissarmyhammer-validators -p mirdan --all-targets -- -D warnings` clean.

    New card ^2r35j9t records the five orphan Rust modules the new scan found in this workspace.
  timestamp: 2026-08-08T17:46:53.806339+00:00
- actor: claude-code
  id: 01kzh7rmst1hnc5xvc12be97kc
  text: |-
    ### implement — changed
    - evidence: 22 files — 5 new rules (`dead-code-{rust,typescript,python,dart,swift}.md`), `unused-code-go.md` and `dead-code.md` rewritten, `unreachable-code-python.md` deleted; 11 new fixtures + `unused-code-go.pass.go.tmpl` and `lib.rs.tmpl` amended, 2 fixtures deleted; `builtin/validators/README.md` and `code-hygiene/VALIDATOR.md`; 3 rosters (`tool_rules.rs`, `builtin/mod.rs`, `mirdan/builtin_validators.rs`). Tests: validators lib 526 pass, mirdan lib 501 pass, cli doctor 17 pass, tools review integration 11 pass, clippy `-D warnings` clean. RED proven 8 ways and restored to GREEN.
    - next: /review
  timestamp: 2026-08-08T17:47:06.426093+00:00
position_column: doing
position_ordinal: '8280'
title: 'dead-code goes objective: tool rules supersede the prompt rule; staged work must be annotated'
---
## STANDING ORDER

This card is a decision, not a proposal. Your job is to implement it exactly.
- Do NOT re-evaluate the tool choices. The evaluation happened. It is recorded here and reversed on ^teemmch.
- Do NOT preserve the prompt rule's judgment. Supersede it as stated.
- Do NOT reject a tool because a zero-config run is noisy. Configure it. Exemptions go in tool configuration or inline suppressions, never prose.
- Do NOT file a follow-up card in place of doing the work.
- Stop and escalate ONLY when a step is impossible. Report the exact command and its output. "It seems risky" is not an escalation.

## The work

Correction to ^teemmch — that card preserved the prompt rule's judgment. The goal is objectivity per the matrix row: dead code = compiler + cargo machete (Rust), knip/ts-prune (TS), vulture (Python), dart analyze unused (Dart), periphery (Swift), compiler (Go). Cover EVERY column. The one subjective carve-out — work-in-process scaffolding — becomes an annotation contract: staged code carries the language's own suppression marker, or it is dead. The compilers already exempt the other carve-outs natively (pub/exported items, cfg(test), main, FFI).

Per language, each rule `supersedes: dead-code` for the files it matches:

- Rust — `dead-code-rust`, workspace scope: `cargo check --message-format=json` piped through jq selecting the `dead_code` lint code. Suppression: `#[expect(dead_code, reason = "...")]` — the staging contract. Plus the orphan-module check the compiler cannot make: a changed `.rs` file that is not `lib.rs`/`main.rs`/`mod.rs`/a `#[path]` target must be named by a `mod` declaration in its crate; grep answers this. (`cargo machete` for unused dependencies is ^s6bn43j, same batch.)
- Go — promote the existing `unused-code-go` to `supersedes: dead-code`. The compiler already errors on unused locals and imports; U1000 covers the rest. Suppression: `//lint:ignore U1000 <reason>`.
- TypeScript — `dead-code-typescript`, workspace scope: `ts-prune` (chosen over knip: it has the inline suppression `// ts-prune-ignore-next` and a narrower, objective claim — unused exports). Map its `path:line - name` output. Pin the version.
- Python — `dead-code-python`: `vulture` per the matrix, default confidence. Suppressions: `--ignore-names`/`--ignore-decorators` in the run script for framework patterns, and vulture's whitelist mechanism for the code's own exemptions; verify current noqa support against the installed tool and state the working suppression in the rule body. `unreachable-code-python` folds into this rule — one owner per finding.
- Dart — `dead-code-dart`, workspace scope: `dart analyze` selecting the `unused_element`, `unused_field`, `unused_import`, `unused_local_variable` diagnostics; keep findings in changed files. Suppression: `// ignore: unused_element` style comments.
- Swift — `dead-code-swift`, workspace scope: `periphery scan --format json`. It needs an SPM/Xcode project and a build; when the project or the tool is absent, doctor reports it and the prompt rule runs — the designed fallback. Suppression: `// periphery:ignore`. Fixtures carry a minimal `Package.swift`, the same shape as the cargo fixture package.

Documentation in the same change:
- Rewrite `dead-code.md` down to the fallback it now is, and state the annotation contract.
- Amend `builtin/validators/README.md` rules-for-tool-rules: an exemption a human would argue in prose must be an inline suppression the tool reads; staging is the canonical example.
- Update the code-hygiene VALIDATOR.md dead-code record: the ^teemmch "do not supersede" decision is reversed, and the knip/periphery/vulture rejections are superseded by this card.

Each rule ships a fail/pass fixture pair (fail = unannotated dead item; pass = the same item with the suppression and a reason). Extend `SHIPPED_DEAD_CODE_RULES` and the acceptance test to every rule.

#tool-validators #dead-code #objectivity