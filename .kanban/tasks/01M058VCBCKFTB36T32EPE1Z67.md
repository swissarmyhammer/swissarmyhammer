---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m05c047ay50mf92jq08035ra
  text: |-
    Measured the tree before writing one sentence. The enumeration is the record below, because two of the card's own claims were themselves stale.

    ## What the tree actually holds

    Enumerated the `name:` frontmatter and the `supersedes:` key of all 84 rule files under `builtin/validators/*/rules/`.

    **25 tool rules ship** (a `tool:` block): 6 `dead-code-*`, 6 `function-length-*`, 5 `magic-numbers-*` (no Rust), 6 `missing-docs-*`, `stuttering-name-go`, `unused-dependencies-rust`. Of the 25, **23 name exactly one prompt rule and 2 name none**. No rule names two. That confirms the card.

    ## Two claims ON THE CARD are stale — do not carry them forward

    1. The card says `magic-numbers` "loads from the USER layer, not from `builtin/validators/`". **`builtin/validators/code-hygiene/rules/magic-numbers.md` ships today** (added by commit ab778d1dc). What the card saw is a leftover: `~/.validators/` on this machine still holds `magic-numbers/` (Jun 19) and `naming/` (Jun 30) — deleted builtin sets that `sah init` no longer refreshes. Only `naming` has no builtin equivalent.
    2. The card says the nearest rule to "No log truncation" is `js-ts/rules/logging.md`. **No such file.** `find` over the whole tree returns exactly one logging rule: `python/rules/logging.md`.

    ## Why the doc bullets were wrong — hard evidence

    `git log --diff-filter=D` over `builtin/validators/**/rules/*.md` shows each named bullet is a rule that was DELETED from builtin:

    - `code-quality/rules/no-string-equality.md` — deleted
    - `code-quality/rules/no-log-truncation.md` — deleted
    - `naming/rules/naming-consistency.md` — deleted (survives only as the stale user-layer copy)
    - `magic-numbers/rules/no-magic-numbers.md` — deleted; today's equivalent is `code-hygiene/magic-numbers`
    - `command-safety/rules/safe-commands.md` — deleted; today's is `code-security/command-safety`. **The card did not name this one; the sweep found it.**

    "No hard-coded values — catches embedded credentials and config" named no rule that ever shipped under that name, and duplicated `code-security/no-secrets`, already listed under Security. "Input validation" names no rule either; the rule is `injection`.

    ## The two card fixes

    - `doc/src/concepts/validators.md` — rewrote the whole Built-in Validators section so each heading is a real SET directory and each bullet is a real rule NAME, greppable in the tree. Removed the three phantom bullets. Added a "Language sets" group stating where naming and logging rules actually live, measured: `js-ts/naming-and-style`, `swift/naming-clarity`, `swift/casing`, `swift/doc-parameter-naming`, `python/logging`.
    - `builtin/validators/README.md` — the `supersedes` sentence now NAMES THE MEMBERS rather than characterising the set: "A shipped tool rule replaces one prompt rule or none: every shipped tool rule but `stuttering-name-go` and `unused-dependencies-rust` names exactly one, and those two declare no `supersedes` key at all". Verified against all 25, not only the two the card names. Checked for guard tests first — `crates/mirdan/src/install/tests.rs` asserts the deployed README equals `include_str!` of the source, so it stays in sync automatically; no test asserts on the sentence text.

    ## What the sweep found beyond the card

    Same cause — prose naming a rule, set, path, or mechanism the code does not hold:

    - `builtin/validators/README.md` — its own example set list named `naming/`, a deleted set. Now `code-hygiene/`.
    - `builtin/validators/code-hygiene/VALIDATOR.md` — "it is the second rule of this set to do that after the `manifests` set's `unused-dependencies-rust`" is self-refuting: `unused-dependencies-rust` is not a rule of this set. `stuttering-name-go` is the only one in `code-hygiene`. Now states "the only rule of this set, and one of two tree-wide".
    - `builtin/validators/swift/VALIDATOR.md` — description enumerated "controlled dependencies", and the body named "the controlled-dependency and Composable Architecture rules" as library-conditional. Both rule files were deleted (`swift/rules/controlled-dependencies.md`, `swift/rules/composable-architecture.md`). Grepped all 11 shipped swift rules for `@Dependency`/`swift-dependencies`/TCA: zero hits, and none opens with a detection clause. Description now enumerates the 11 shipped rules; the body states no rule is library-scoped.
    - `builtin/validators/rust/VALIDATOR.md` — description enumerated "future-proofing" and "documentation idioms"; `rust/rules/future-proofing.md` and `rust/rules/documentation.md` are both deleted. Now enumerates the 4 shipped rules.
    - `doc/src/concepts/validators.md` — the `VALIDATOR.md` example declared a SET named `dead-code`, which is a RULE of `code-hygiene`; a reader would hunt a `dead-code/` set that ships nowhere. Now `my-team-rules`.
    - `doc/src/concepts/validators.md` (x2) — claimed the user store is `$XDG_DATA_HOME/validators/` (default `~/.local/share/validators/`). `user_validators_dir()` is `dirs::home_dir().join(ValidatorsConfig::DIR_NAME)` and `DIR_NAME = ".validators"`; no validator path reads XDG. Both now `~/.validators/`, matching the shipped README.
    - `doc/src/concepts/integrated-sdlc.md` — the layer diagram and the phase table named sets `code-quality` and `security`, neither of which exists, and listed `command-safety` (a RULE of `code-security`) in a column of validator sets. Also `.avp/validators/` for the project store, which is `.validators`.
    - `doc/src/concepts/integrated-sdlc.md` + `doc/src/concepts/tools.md` — both described a RETIRED execution model as live: "Validators are Claude Code hooks that fire on every tool call" and "check each tool invocation before it executes". `crates/swissarmyhammer-validators/src/lib.rs` states plainly: "The hook-triggered execution path (per-tool-call validator dispatch via PreToolUse/PostToolUse/Stop hooks) has been retired." Both now describe the review pipeline over changed files, and the SDLC page says outright that per-tool-call dispatch is gone. This was fixed rather than deferred because it sits inside the same sentences that named `command-safety` as a validator — leaving it would have been a half-fix of the same cause.

    Not a stray: `crates/swissarmyhammer-mcp-proxy/.kanban` is git-tracked and dated Jul 5, a committed fixture. This session created no directory outside the repo root.

    ### implement — changed
    - evidence: 7 files — builtin/validators/README.md, builtin/validators/code-hygiene/VALIDATOR.md, builtin/validators/rust/VALIDATOR.md, builtin/validators/swift/VALIDATOR.md, doc/src/concepts/validators.md, doc/src/concepts/integrated-sdlc.md, doc/src/concepts/tools.md. `cargo fmt` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean; `cargo nextest run --workspace` 14105 passed, 0 failed, 0 skipped; `cargo build -p mirdan` OK and the generated `builtin_validators.rs` embed carries the edited README text.
    - next: /review. Left in `doing`, not committed.
  timestamp: 2026-08-16T13:25:54.538897+00:00
- actor: claude-code
  id: 01m05ch3968gqqba7cbppqac52
  text: |-
    ### test — green

    Scope: 7 prose-only files (README.md, three VALIDATOR.md, three doc/src/concepts/*.md). No Rust source changed.

    Gates:
    - `cargo fmt --check` — exit 0, no diff.
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, zero warnings.
    - `cargo nextest run --workspace` — 14105 tests run: 14105 passed (63 slow), 0 failed, 0 skipped.

    Embed check (load-bearing, per task instructions):
    - `cargo build -p mirdan` succeeded. Traced the actual build-script OUT_DIR for the `mirdan` package (`target/debug/build/mirdan-2dd4355981eab855/out/builtin_validators.rs`, confirmed via `cargo build -p mirdan -v` showing the `build-script-build` invocation, then locating the freshly-written `out/` dir by mtime after touching `build.rs` to force a rerun).
    - Confirmed the new wording is present in that embed: the README `supersedes` correction ("prompt rule or none: every shipped tool rule but `stuttering-name-go` and..."), the code-hygiene `stuttering-name-go` rule body ("It is the only rule of this set to do that, and one of two tree-wide..."), the rust VALIDATOR.md description ("design, and trait implementations applied to changed Rust files."), and the swift VALIDATOR.md description/body ("two sources: Apple's **Swift API Design Guidelines** and the idioms of Apple's...", "Every rule here reads plain Swift...").
    - `crates/mirdan/src/install/tests.rs` asserts deployed README == `include_str!` of `builtin/validators/README.md`. Ran targeted: `cargo nextest run -p mirdan install::` — 105 passed (2 leaky, not failures), 398 skipped-by-filter (expected, this was a `-p mirdan` scoped run, not the full suite — the full workspace run above covers all of them at 0 skipped).

    VALIDATOR.md frontmatter check:
    - code-hygiene: `name: code-hygiene`, `description`, `metadata.version`, `match.files: ["@file_groups/source_code"]`, `probes: [callers]` — all present, only body prose changed (the diff touched lines 386-392, past the frontmatter block).
    - rust: `name: rust`, `match.files: ["**/*.rs"]` — only `description` prose changed.
    - swift: `name: swift`, `match.files: ["**/*.swift"]` — only `description` prose and one body paragraph changed.
    - None of the four VALIDATOR.md files carry `severity` or `tags` keys at the manifest level (those live on individual rule files, untouched by this diff) — grepped for `^severity:`/`^tags:` in code-hygiene/VALIDATOR.md, no match, consistent with the README's own documented schema (`name`, `description`, `match`, `probes` only at the set-manifest level).
    - Grepped the tree for the removed old wording (`dtolnay school` co-occurring with `trait impls, future-proofing`, `Point-Free`, `library-conditional`, `three sources`, `Composable Architecture`, `casing, naming clarity`) — no loader test, roster guard, or other source file asserts on the removed text. The only stray hits were unrelated: `dtolnay school` still legitimately present in the new rust description, and `three sources` in `crates/mirdan/src/install/package.rs` is an unrelated comment about MCP config resolution order.

    Rule-name existence check (every rule name written into the corrected prose, checked against `builtin/validators/*/rules/*.md` on disk):
    - code-hygiene: `function-length`, `magic-numbers`, `missing-docs`, `no-commented-code`, `dead-code`, `data-driven` — all exist. Tool rules named: `function-length-rust`, `magic-numbers-python`, `missing-docs-typescript`, `stuttering-name-go` — all exist.
    - code-security: `no-secrets`, `injection`, `command-safety` — all exist.
    - test-integrity: `no-test-cheating`, `no-hard-code` — all exist.
    - completeness: `invariant-propagation`, `inverse-operation-coverage`, `case-sensitivity-coverage`, `public-output-contract` — all exist.
    - duplication/reuse: `duplication`, `reuse`, plus the named carve-outs `duplication/rules/rust.md`, `duplication/rules/swift.md` — all exist.
    - manifests: `unused-dependencies-rust` — exists.
    - Language sets named by rule: `js-ts/naming-and-style`, `swift/naming-clarity`, `swift/casing`, `swift/doc-parameter-naming`, `python/logging` — all exist.
    - README.md and code-hygiene/VALIDATOR.md both name `stuttering-name-go` and `unused-dependencies-rust` as the two rules with no `supersedes` — both exist and both files carry no `supersedes:` key, matching the corrected claim.
    - No rule name in the diff failed to resolve to a file under `builtin/validators/*/rules/`.

    No stale text was restored. Did not commit; task left in `doing`.
  timestamp: 2026-08-16T13:35:10.630460+00:00
position_column: doing
position_ordinal: '8280'
title: Two more stale claims found beside the complexity scorer sweep
---
Found while doing ^6dx6dkz. Neither names the deleted complexity scorer, so neither was in that card's scope. Both are stale claims that a reader would act on.

## 1. `doc/src/concepts/validators.md` names rules that do not ship

The "Code Quality" list carries these bullets:

- **No string equality** — no rule of that name ships in `builtin/validators/`.
- **No log truncation** — no rule of that name ships either. The nearest is `js-ts/rules/logging.md`, which is a different rule for one language.
- **Naming consistency** — no builtin rule of that name. `swift/naming-clarity`, `swift/doc-parameter-naming` and `js-ts/naming-and-style` ship instead, each for one language.
- **No magic numbers**, **No hard-coded values** — `magic-numbers` and `naming` load from the USER layer, not from `builtin/validators/`. The page presents the whole list as "Built-in Validators".

The page does say the groups are "illustrative rather than exhaustive", which covers a list that is SHORT. It does not cover a list that names rules the tree does not hold.

## 2. `builtin/validators/README.md` states every tool rule supersedes exactly one prompt rule

The `supersedes` section closes with: "No shipped rule names two today: each shipped tool rule replaces exactly one prompt rule."

The second half is false. `builtin/validators/code-hygiene/rules/stuttering-name-go.md` declares no `supersedes` key at all, and neither does the `manifests` rule `unused-dependencies-rust`. The test roster agrees: `SHIPPED_STUTTERING_NAME_RULES` and `SHIPPED_UNUSED_DEPENDENCY_RULES` in `crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs` both name `SUPERSEDES_NOTHING`, whose own doc states "No shipped prompt rule asks whether a declared dependency is used, so this group replaces no rule and degrades to no rule."

So a shipped tool rule can replace one prompt rule or none. The first half of the sentence — no rule names two — still holds.

## Done when

- Each bullet in the `doc/src/concepts/validators.md` Code Quality list names a rule the validator tree actually holds, or the page says which layer the rule comes from.
- The `builtin/validators/README.md` sentence states that a shipped tool rule replaces one prompt rule or none, and names the two rules that replace none.
- `cargo nextest run --workspace` green; fmt and clippy clean.

#tool-validators