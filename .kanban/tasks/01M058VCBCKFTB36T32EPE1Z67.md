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
- actor: claude-code
  id: 01m05ck9sj0qejn7wzt58rtt55
  text: |-
    ### commit — changed
    - evidence: 21cb8b128 docs(validators): remove two more stale claims from validator prose (^epe1z67); 9 files changed (7 prose + 2 new .kanban files for this task)
    - next: none
  timestamp: 2026-08-16T13:36:22.834584+00:00
- actor: claude-code
  id: 01m05cw168khnqsnsk3mx0bbp4
  text: |
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (21cb8b128) returned 0 findings but matched 0 of the 7 changed prose files (only the 2 `.kanban` files were seen, both ignore-excluded), so the engine report was empty and every written claim was verified directly. 1 finding — builtin/validators/code-hygiene/VALIDATOR.md:389
    - verified clean: 25 tool rules ship (frontmatter `tool:` key), 23 supersede exactly one prompt rule, 2 supersede none (`stuttering-name-go`, `unused-dependencies-rust`), none names two — README sentence holds. Every rule name written into the corrected prose exists under `builtin/validators/*/rules/` (all 13 sets covered, 84 rule files). User store is `~/.validators/` via `ValidatorsConfig::DIR_NAME` + `from_user_home` (crates/swissarmyhammer-directory/src/config.rs) — the `$XDG_DATA_HOME/validators/` text was correctly removed. Per-tool-call hook dispatch is genuinely retired (crates/swissarmyhammer-validators/src/lib.rs:3-4), and the `trigger` key claim checks out against `LEGACY_MANIFEST_KEYS` (crates/swissarmyhammer-validators/src/validators/parser.rs:335) — `trigger` is a real removed key, distinct from the still-live `triggerMatcher`. `unused-dependencies-rust` belongs to `manifests`; `code-hygiene` holds exactly one tool rule that supersedes nothing. Swift "no rule is scoped to a third-party library" holds — no detection clause in any swift rule. Rust description now lists exactly its 4 rules; swift lists exactly its 11. Three VALIDATOR.md frontmatter blocks changed (not four — the fourth builtin file is README.md, which carries no frontmatter); all three fences intact.
    - line accounting: 226/66 is 129 insertions of `.kanban` card body + 97/66 of actual prose; the net +31 is the validators.md rule listing expanding from short lists to full per-set enumerations (+56/-29).
    - next: qualify the `code-hygiene/VALIDATOR.md` counts with "tool rule", then re-review.
  timestamp: 2026-08-16T13:41:08.936265+00:00
- actor: claude-code
  id: 01m05cxhf2yx55fejg9gqhw2pz
  text: |
    ### finish iteration 1 — findings
    - implement: changed — 7 files, all prose. Root cause found and stated: every phantom bullet is a rule DELETED from builtin, proved with `git log --diff-filter=D` for no-string-equality, no-log-truncation, naming-consistency, no-magic-numbers and safe-commands. All 84 rule files were enumerated BEFORE anything was written. The sweep found 8 instances beyond the 2 the card named, and corrected two claims on the card itself that were stale.
    - test: green — cargo fmt --check exit 0; cargo clippy --workspace --all-targets -- -D warnings exit 0, 0 warnings; cargo nextest run --workspace 14105 passed, 0 failed, 0 skipped; cargo build -p mirdan succeeds and the corrected prose was confirmed present in the traced OUT_DIR embed; the README include_str! round-trip test passes; every rule name in the corrected prose verified to exist.
    - commit: 21cb8b128 docs(validators): remove two more stale claims from validator prose (^epe1z67) — 9 files changed, 226 insertions, 66 deletions
    - review: findings — 1 finding, builtin/validators/code-hygiene/VALIDATOR.md:389.

    ### The finding — a dropped qualifier makes two counts false

    The replacement sentence reads "It supersedes nothing. It is the only rule of this set to do that, and one of two tree-wide". Both counts are false without the word `tool`:

    - SEVEN `code-hygiene` rules declare no `supersedes` key (data-driven, dead-code, function-length, magic-numbers, missing-docs, no-commented-code, stuttering-name-go), not one.
    - Tree-wide, 61 of 84 rule files declare none, not two.

    Both counts hold only of TOOL rules. The same commit's README gets this right — "every shipped tool rule but ... names exactly one" — so the correct wording already exists a few files away. This is the card's own defect class, in SHIPPED validator content that reaches users.

    ### The review engine gave ZERO signal on this card

    Worth recording as a structural fact, not as a complaint. The engine matched 0 of the 7 changed prose files. It saw only the 2 `.kanban` files, which `.reviewignore` excludes. So its "0 findings" carried no information, and the single finding came entirely from verifying each written claim by hand against the tree.

    Four of the seven files are `builtin/validators/**`, which is SHIPPED content embedded into the binary. An error there reaches every user. The engine reviews none of it.

    ### Verified correct by the review

    - 25 tool rules ship; 23 supersede exactly one; 2 supersede none; none names two. Enumerated independently, so the README sentence holds.
    - Every rule name in the corrected prose exists under `builtin/validators/*/rules/`; all 13 sets and 84 rule files accounted for, and no name the tester missed.
    - The user store really is `~/.validators/` via `ValidatorsConfig::DIR_NAME`, so removing the `$XDG_DATA_HOME` text was right.
    - The per-tool-call hook path really is retired per `swissarmyhammer-validators/src/lib.rs:3-4`, so rewriting the hook description was in scope and accurate.
    - The `trigger` key claim survives scrutiny: `LEGACY_MANIFEST_KEYS` confirms `trigger` is a genuinely removed key, distinct from the live `triggerMatcher`.
    - Swift's "no rule is scoped to a third-party library" holds; the one `swift-nio` mention is attribution rather than a detection clause.
    - Three `VALIDATOR.md` files changed, not four — `README.md` carries no frontmatter. All three frontmatter fences intact.
    - 226/66 accounted for: 129 insertions are this card's board body, leaving 97/66 of real prose. The net +31 is validators.md expanding illustrative lists into full per-set enumerations.
    - next: iteration 2 works the open finding
  timestamp: 2026-08-16T13:41:58.370045+00:00
- actor: claude-code
  id: 01m05dh1m1xkpwk60jrc8qrmnp
  text: |-
    Worked the one open finding, then removed its CAUSE from the whole change rather than the quoted line alone.

    ## The counts, measured again before writing

    Enumerated `builtin/validators/*/rules/*.md` and tested each file for a `^supersedes:` key and a `^tool:` key.

    - 84 rule files ship. 23 declare `supersedes`. 61 declare none.
    - 25 tool rules ship. 23 of the 25 declare `supersedes`. 2 declare none.
    - Every rule that declares `supersedes` is a tool rule. No prompt rule carries the key.
    - `code-hygiene` holds 24 tool rules and 6 prompt rules. `stuttering-name-go` is the ONLY code-hygiene tool rule with no `supersedes`. The other rule with none tree-wide is `manifests/unused-dependencies-rust`.

    So the finding's arithmetic holds: "only rule of this set" and "one of two tree-wide" are false, and both are true of TOOL rules.

    Also traced WHY only a tool rule can carry the key, so the corrected prose states a mechanism and not a number that goes stale. `matched_tool_rules` in `crates/swissarmyhammer-validators/src/review/tool_rules.rs` skips a rule whose `tool` field is `None`, so `supersedes` on a prompt rule would never be read.

    ## The one instance the finding names

    `builtin/validators/code-hygiene/VALIDATOR.md`, the "## Naming" section. Both counts now carry the `tool` qualifier, and one sentence states the mechanism behind them: the engine reads `supersedes` on a rule with a `tool` block and nowhere else, so a prompt rule enters neither count.

    ## Three more instances of the SAME cause, found by re-reading every sentence the card wrote

    1. **`builtin/validators/code-hygiene/VALIDATOR.md`, same paragraph** — "the naming rules that ship are `swift/naming-clarity`, `swift/doc-parameter-naming` and `js-ts/naming-and-style`" is a false exhaustive claim. `swift/casing` reads names too — its own description is "UpperCamelCase types, lowerCamelCase members, uniform acronym casing, no Hungarian/k-prefix". The same commit's `doc/src/concepts/validators.md` already gets this right and lists `casing` among the swift naming rules. Corrected to name all four prompt rules written about naming, and "none of the three" is now "none of the four". The conclusion is unchanged: none of the four reads a `.go` file.

    2. **`builtin/validators/README.md`** — "A rule that supersedes nothing replaces no rule and degrades to no rule" is the exact defect: a claim about tool rules stated as a claim about rules. It is FALSE of a prompt rule — `code-hygiene/dead-code` and `no-commented-code` supersede nothing and do not degrade to no rule, they ARE the rule and they run. Now "A TOOL rule that supersedes nothing replaces no prompt rule and degrades to no rule", with the mechanism sentence after it. The 25/23/2 counts in the sentence above were NOT touched; the review settled those.

    3. **`doc/src/concepts/validators.md`, "Language sets"** — "A naming or logging rule lives here rather than in `code-hygiene`" is refuted by the same page four paragraphs above, which names `stuttering-name-go` as a code-hygiene rule that flags a Go NAME. Now qualified to "prompt rule", with a sentence stating that a naming TOOL rule can live in `code-hygiene` and one does.

    ## One instance of the same SHAPE, in a different qualifier

    `doc/src/concepts/integrated-sdlc.md` said "planning changes none" and "none — planning changes no file". Planning DOES change files: it writes `.kanban/tasks/*.md` and `*.jsonl` cards. The true claim needs the glob qualifier. Measured the match blocks of all 13 sets plus `builtin/file_groups/source_code.yaml` (33 patterns) and `test_files.yaml` (24 patterns): no shipped validator glob matches `.md` or `.jsonl`. The prose now says planning writes only kanban cards that no shipped validator glob matches, and the table cell says "no file a validator glob matches".

    ## Checked and left alone

    - `builtin/validators/rust/VALIDATOR.md` and `builtin/validators/swift/VALIDATOR.md` — the 4-rule and 11-rule description enumerations are exact, and the swift set holds no tool rule, so "Every rule here reads plain Swift" needs no qualifier. Settled by the review.
    - `doc/src/concepts/tools.md` — one line, and it writes "e.g.", so it makes no exhaustive claim.
    - README "No shipped rule names two today" — TRUE as written, because no prompt rule carries the key at all. Left alone rather than churned.
    - The linter counts in the naming survey (revive's 12 `naming` rules, golangci-lint's 115 linters) are counts of LINTERS, not of validator rules, so the `tool` qualifier does not reach them.

    ## Embed confirmed

    `cargo build -p mirdan` reran the build script. The traced embed `target/debug/build/mirdan-2dd4355981eab855/out/builtin_validators.rs` carries "only TOOL rule of this set", "A TOOL rule that" and the four-rule naming list, and holds ZERO copies of the old "the only rule of this set to do that". `builtin/validators/**` is shipped content, so this was the load-bearing check.

    No stray dotdir was created. `crates/swissarmyhammer-mcp-proxy/.kanban` is the git-tracked fixture already recorded on this card.

    ### implement — changed
    - evidence: 4 files — builtin/validators/code-hygiene/VALIDATOR.md, builtin/validators/README.md, doc/src/concepts/validators.md, doc/src/concepts/integrated-sdlc.md. Measured 84 rule files / 23 with `supersedes` / 61 without / 25 tool rules / 23 tool rules with `supersedes` / 2 without. `cargo fmt` clean; `cargo clippy --workspace --all-targets -- -D warnings` exit 0, zero warnings; `cargo nextest run --workspace` 14105 passed, 0 failed, 0 skipped; `cargo build -p mirdan` OK with the corrected text confirmed in the embed.
    - next: /review. Left in `doing`, not committed.
  timestamp: 2026-08-16T13:52:37.505171+00:00
- actor: claude-code
  id: 01m05dx556kd6cw54gk22sksxs
  text: |-
    ### test — green

    Scope: iteration 2, 4 prose files — builtin/validators/code-hygiene/VALIDATOR.md, builtin/validators/README.md, doc/src/concepts/validators.md, doc/src/concepts/integrated-sdlc.md. No Rust source changed. Confirmed via `git diff --stat` that exactly these 4 files (plus kanban card files) are dirty.

    Gates:
    - `cargo fmt --check` — exit 0, no diff.
    - `cargo clippy --workspace --all-targets -- -D warnings` — exit 0, zero warnings.
    - `cargo nextest run --workspace` — 14105 tests run: 14105 passed, 0 failed, 0 skipped.
    - `cargo build -p mirdan` — succeeded after touching `crates/mirdan/build.rs` to force a rerun. Traced the freshest OUT_DIR by mtime: `target/debug/build/mirdan-2dd4355981eab855/out/builtin_validators.rs` (confirmed newer than build.rs via `find -newer`).

    Embed check (load-bearing): `crates/mirdan/build.rs` embeds only `../../builtin/validators` (confirmed by reading the build script), so only 2 of the 4 files — `builtin/validators/README.md` and `builtin/validators/code-hygiene/VALIDATOR.md` — reach the binary; `doc/src/concepts/*.md` correctly do NOT appear in the embed (`grep -c "doc/src/concepts"` → 0), matching the card's own framing.
    - code-hygiene/VALIDATOR.md: embed contains verbatim "It is the only TOOL rule of this set to do that, and one of two tool rules tree-wide — the other is the `manifests` set's `unused-dependencies-rust`. Both counts are of tool rules only." and "the prompt rules written about naming are `swift/casing`, `swift/naming-clarity`, `swift/doc-parameter-naming` and `js-ts/naming-and-style`, and none of the four reads a `.go` file." Old wording ("only rule of this set to do that", "the naming rules that ship are ... and none of the three") absent — the one "none of the three" hit in the embed is an unrelated fixture comment about a different rule, not stale prose.
    - README.md: embed contains verbatim "A TOOL rule that supersedes nothing replaces no prompt rule and degrades to no rule ... A prompt rule never carries the key — the engine reads `supersedes` on a rule that carries a `tool` block and nowhere else."

    Independently re-counted every claim (did not trust the implementer's numbers):
    - 84 total rule files under `builtin/validators/*/rules/*.md` — confirmed (`find ... | wc -l` → 84).
    - 23 declare `supersedes:`, 61 do not — confirmed (`grep -l`/`grep -L` counts → 23 / 61).
    - 25 declare `tool:` (tool rules) — confirmed.
    - Of the 25 tool rules, 23 declare `supersedes` and exactly 2 do not: `builtin/validators/code-hygiene/rules/stuttering-name-go.md` and `builtin/validators/manifests/rules/unused-dependencies-rust.md` — confirmed via `comm` set difference, output was exactly those two paths.
    - Zero prompt rules (no `tool:` key) declare `supersedes` — confirmed via `comm` set difference, empty output.
    - `code-hygiene` holds 30 rule files total: 24 with `tool:`, 6 without — confirmed by direct listing.
    - Naming enumeration now names 4 rules: `swift/naming-clarity`, `swift/doc-parameter-naming`, `swift/casing`, `js-ts/naming-and-style` — all 4 exist as files (checked each path individually).
    - `integrated-sdlc.md` glob claim: enumerated all 13 `builtin/validators/*/VALIDATOR.md` `match:` blocks plus `builtin/file_groups/source_code.yaml` and `builtin/file_groups/test_files.yaml` — no glob pattern in any of the 15 sources matches `.md` or `.jsonl`.
    - `code-hygiene/VALIDATOR.md` frontmatter: read the first 15 lines — `---` opens and closes the block, `name`, `description`, `metadata.version`, `match.files`, `probes` all present and unchanged; only body prose past the frontmatter differs.

    No stale text was restored to make anything pass. Did not commit; task left in `doing`.
  timestamp: 2026-08-16T13:59:14.342999+00:00
- actor: claude-code
  id: 01m05dzkwg7yvcgwfww075d1ng
  text: |-
    ### commit — changed
    - evidence: 12300d8eb fix(validators): put the tool qualifier back on two counts (^epe1z67); 6 files changed, 170 insertions(+), 15 deletions(-)
    - next: none
  timestamp: 2026-08-16T14:00:34.960918+00:00
- actor: claude-code
  id: 01m05eaqgrsaky4btetxnf2a9r
  text: |
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (commit 12300d8eb) returned findings 0, confirmed 0, refuted 0 — it matched 0 of the 4 changed prose files and skipped the 2 `.kanban/` files by `.reviewignore`, so the claims were verified by hand against the tree. 1 finding — builtin/validators/code-hygiene/VALIDATOR.md:395. The two corrected counts, the `supersedes` mechanism claim, and the `integrated-sdlc.md` glob claim are all true as written; the naming enumeration is still not exhaustive and omits `swift/fluent-usage`.
    - next: replace the closed enumeration at VALIDATOR.md:395 with the structural fact — no shipped language set covers Go, and the six sets that match `**/*.go` hold no prompt rule about what a name is.
  timestamp: 2026-08-16T14:06:39.128890+00:00
- actor: claude-code
  id: 01m05ecv27g44q95yy98wyb8yg
  text: |
    ### finish iteration 2 — findings
    - implement: changed — 4 files. Counts measured before writing: 84 rule files, 23 declare `supersedes`, 61 do not, 25 tool rules, 23 of those declare it, 2 do not. Found and fixed 4 further instances of the same cause beyond the finding.
    - test: green — cargo fmt --check exit 0; cargo clippy --workspace --all-targets -- -D warnings exit 0, 0 warnings; cargo nextest run --workspace 14105 passed, 0 failed, 0 skipped; cargo build -p mirdan succeeds and the traced OUT_DIR embed carries the corrected text with the stale wording absent; every count independently re-counted by the tester.
    - commit: 12300d8eb fix(validators): put the tool qualifier back on two counts (^epe1z67) — 6 files changed, 170 insertions, 15 deletions
    - review: findings — 1 finding, builtin/validators/code-hygiene/VALIDATOR.md:395.

    ### Everything the finding of iteration 1 asked for is now CORRECT

    The reviewer re-measured with its own script rather than reading the tester's numbers: 84 rule files, 23 with `supersedes`, 61 without, 25 tool rules, 23 of those with it, 2 without, and `code-hygiene` at 30 rules — 24 tool and 6 prompt. "The only TOOL rule of this set" and "one of two tool rules tree-wide" both hold exactly.

    The mechanism claim holds too: `tool_rules.rs:507` reads `let Some(spec) = &rule.tool else { continue; };`, so a rule with `tool: None` never reaches the suppression walk. 0 of 59 shipped prompt rules declare `supersedes`.

    The `integrated-sdlc.md` correction is accurate against all 13 set match blocks and both file-group YAMLs. The 170/15 resolves: 149 insertions are the board record, leaving 21/14 of real prose.

    ### The new finding — a CLOSED enumeration that is not exhaustive

    `builtin/validators/code-hygiene/VALIDATOR.md:395` states "the prompt rules written about naming are `swift/casing`, `swift/naming-clarity`, `swift/doc-parameter-naming` and `js-ts/naming-and-style`, and none of the four reads a `.go` file".

    `swift/fluent-usage` is a fifth prompt rule about naming and is omitted — every bullet of it dictates what a declaration is called. `rust/api-design` and `js-ts/react-components` are omitted too. The word "four" CLOSES the list, so the sentence is false as written.

    The conclusion survives: none of the omitted rules reads a `.go` file either.

    ### The prescription — remove the cause, do not add a fifth name

    The reviewer is explicit, and this is the important part. "Rules written about naming" has NO mechanical test. It must be recounted by judgment every time a rule is added, so it will go stale again. Adding `swift/fluent-usage` fixes this instance and leaves the cause.

    The fix is to state a structurally checkable fact instead: no shipped language set covers Go, and the sets that match `**/*.go` through `@file_groups/source_code` hold no prompt rule about what a name is.

    ### Guardrail state

    Two rounds, two findings, both in `code-hygiene/VALIDATOR.md`, both of the same CLASS — a claim wider than what the tree supports — but at different lines with different content:

    1. iteration 1 — line 389, a dropped `tool` qualifier making two counts false. ANSWERED, verified correct.
    2. iteration 2 — line 395, a closed four-item enumeration missing a fifth member.

    Not the same file:line and not the same message, so the guardrail is not tripped. The loop continues, and iteration 3 must fix the CAUSE rather than the instance.
    - next: iteration 3 works the open finding
  timestamp: 2026-08-16T14:07:48.295563+00:00
- actor: claude-code
  id: 01m05f2qngh8n36a18n8wkfj7e
  text: |-
    Worked the one open finding. Removed the CAUSE — a membership test that needs judgment — rather than the instance, and applied the same standard to every other closed enumeration this card wrote over 21cb8b128 and 12300d8eb.

    ## The replacement statement, and how it was verified

    The old sentence asked the reader to decide which rules are "written about naming". No command answers that, so the list went stale two rounds running. The new sentence asks which sets a `.go` file REACHES, which one command answers.

    `builtin/validators/code-hygiene/VALIDATOR.md`, the "## Naming" section, now reads:

    > No shipped prompt rule reads a Go NAME, and the check for that is a `match` block rather than a reading of what each rule is about. Seven of the thirteen shipped sets declare their own globs, and no glob among them names Go — `dart` matches `**/*.dart`, `js-ts` matches `**/*.js`, `**/*.jsx`, `**/*.ts` and `**/*.tsx`, `numpy` and `python` each match `**/*.py`, `rust` matches `**/*.rs`, `swift` matches `**/*.swift`, and `manifests` matches `**/Cargo.toml`. A `.go` file therefore reaches the other six sets alone — `code-hygiene`, `code-security`, `completeness`, `duplication`, `reuse` and `test-integrity` — each through the `*.go` entry of `@file_groups/source_code`, whose patterns match at any depth. Those six sets hold 19 prompt rules between them, which is one directory listing filtered on the `tool` frontmatter key, and `stuttering-name-go` is the only rule of the six that reads a name at all. A reader checks that with a command rather than by deciding what a rule is about. A machine without `revive` therefore gets no answer to this question rather than a worse one.

    Every number was read off the tree before it was written, from the FRONTMATTER of each `VALIDATOR.md` rather than from the file body:

    - 13 sets ship (`ls -d builtin/validators/*/`).
    - 7 declare their own `match.files` globs: `dart` `**/*.dart`; `js-ts` `**/*.js` `**/*.jsx` `**/*.ts` `**/*.tsx`; `manifests` `**/Cargo.toml`; `numpy` `**/*.py`; `python` `**/*.py`; `rust` `**/*.rs`; `swift` `**/*.swift`. Grepped all 7 frontmatter blocks for `.go`: zero hits.
    - The other 6 declare `@file_groups/...`: `code-hygiene`, `code-security`, `duplication` and `reuse` take `source_code`; `completeness` and `test-integrity` take `source_code` plus `test_files`.
    - 19 prompt rules across those 6 — `ls .../rules/*.md | xargs grep -L '^tool:' | wc -l`. They are `code-hygiene`'s `data-driven`, `dead-code`, `function-length`, `magic-numbers`, `missing-docs`, `no-commented-code`; `code-security`'s `command-safety`, `injection`, `no-secrets`; `completeness`'s `case-sensitivity-coverage`, `invariant-propagation`, `inverse-operation-coverage`, `public-output-contract`; `duplication`'s `duplication`, `rust`, `swift`; `reuse`'s `reuse`; `test-integrity`'s `no-hard-code`, `no-test-cheating`. Each carries an empty rule-level `match`, so a rule-level narrowing takes none of them out of Go's reach.

    One correction to the reviewer's own wording, made rather than copied. The group's Go entry is `*.go`, not `**/*.go`. It still matches at any depth, because `GLOB_MATCH_OPTIONS` in `crates/swissarmyhammer-validators/src/validators/types.rs` sets `require_literal_separator: false`, so `*` crosses `/`. The prose states the pattern the YAML actually holds and says why it reaches a nested path.

    The residual "reads a name at all" now sits over 19 named rules a reader can list with one command. It is no longer a judgment over 84 files, and the count 19 is the tripwire: a rule added to any of the six breaks the number mechanically.

    ## The other closed enumerations of the same class, found and fixed

    Re-read every sentence the two commits wrote and tested each list for the defect: closed by a count word or by "the"/"all"/"none", with a membership test that needs judgment.

    1. **`builtin/validators/README.md`, the `supersedes` section.** It carried "those two declare no `supersedes` key at all — no shipped prompt rule reads a Go NAME, and no shipped prompt rule asks whether a declared dependency is used." Two "none of the ..." claims over all 59 shipped prompt rules, each answered by judgment. Both now follow from a `match` block: a `Cargo.toml` reaches the `manifests` set alone, and `builtin/validators/manifests/rules/` holds `unused-dependencies-rust.md` and NOTHING ELSE, so there is no prompt rule in the only set that file reaches. The Go half points at the derivation in `code-hygiene/VALIDATOR.md` rather than repeating it. Verified: `manifests` is the only set whose `match.files` names `Cargo.toml`; `source_code.yaml` and `test_files.yaml` declare no `*.toml` pattern.

    2. **`doc/src/concepts/integrated-sdlc.md`, the phase table.** The Validators column named a different set list for each phase — `code-hygiene, code-security` for Implement, `test-integrity, code-hygiene` for Test, `code-hygiene, code-security, completeness, duplication, reuse` for Review. Membership is pure judgment, and the mechanism refutes it: the engine picks a set by glob, not by phase, which the SAME page states two sections down. The three middle cells now read "every set whose globs match the changed files", and a sentence under the table says the column names no set because the engine picks none by phase, with `rust` for a `.rs` change and `js-ts` for a `.tsx` one as the example. The Plan and Commit cells were left alone — the review settled the Plan glob correction, and Commit changes no file.

    ## Checked against the same standard and deliberately left alone

    - `README.md` "every shipped tool rule but `stuttering-name-go` and `unused-dependencies-rust` names exactly one", and "No shipped rule names two today" — closed, but membership is `grep '^supersedes:'` crossed with `grep '^tool:'`. Mechanical, and settled by the review.
    - `code-hygiene/VALIDATOR.md:389` — the "only TOOL rule of this set" and "one of two tool rules tree-wide" counts. Mechanical, settled by the review. Not touched.
    - `doc/src/concepts/validators.md:77`, the Language sets paragraph — the reviewer ruled this one open, and re-reading confirms it: no count word, no "the naming rules are", and the section preamble says the lists are illustrative rather than exhaustive. `swift/fluent-usage` being absent does not make it false. Left.
    - `doc/src/concepts/validators.md` per-set bullet lists, and the "Four of these — `dead-code`, `function-length`, `magic-numbers` and `missing-docs`" sentence — each is a directory listing or a `supersedes` value set. Mechanical. Left.
    - `swift/VALIDATOR.md` and `rust/VALIDATOR.md` descriptions — each enumerates its own `rules/` directory. Mechanical, settled by the review. Left.
    - `doc/src/concepts/tools.md` — writes "e.g.", so it closes nothing. Left.
    - The 84/23/61/25/23/2 counts, the `code-hygiene` 24 tool + 6 prompt split, the `tool_rules.rs:507` mechanism claim and the `integrated-sdlc.md` glob correction were NOT re-churned, per the card.

    ## Embed confirmed

    `builtin/validators/**` is shipped content, so the embed is the load-bearing check. Touched `crates/mirdan/build.rs` to force the build script to rerun, then traced the fresh OUT_DIR by `-newer`: `target/debug/build/mirdan-2dd4355981eab855/out/builtin_validators.rs`. It carries "Seven of the thirteen", "reaches the other six sets alone", "19 prompt rules between them" and "reaches the `manifests` set alone", and it holds ZERO copies of "the prompt rules written about naming are", "none of the four reads" or "no shipped prompt rule reads a Go NAME, and no".

    No stray dotdir was created. `crates/swissarmyhammer-mcp-proxy/.kanban` is the git-tracked fixture already recorded on this card. `git status` shows exactly three prose files dirty.

    ### implement — changed
    - evidence: 3 files — builtin/validators/code-hygiene/VALIDATOR.md, builtin/validators/README.md, doc/src/concepts/integrated-sdlc.md. Measured 13 sets / 7 with own globs, none naming Go / 6 reaching `.go` through `@file_groups/source_code` / 19 prompt rules across those 6 / `manifests` holding 1 rule and it a tool rule. `cargo fmt --check` exit 0; `cargo clippy --workspace --all-targets -- -D warnings` exit 0, zero warnings; `cargo nextest run --workspace` 14105 passed, 0 failed, 0 skipped; `cargo build -p mirdan` OK with the corrected text confirmed in the traced OUT_DIR embed and the stale text absent.
    - next: /review. Left in `doing`, not committed.
  timestamp: 2026-08-16T14:19:45.712201+00:00
- actor: claude-code
  id: 01m05fg5s7mzd7w4r8d1758scf
  text: |-
    ### test — green

    **Gates**
    - `cargo fmt --check`: exit 0, no diff.
    - `cargo clippy --workspace --all-targets -- -D warnings`: exit 0, zero warnings. Compiled mirdan, mirdan-app, swissarmyhammer-tools, mirdan-cli, swissarmyhammer-agent, kanban-app, kanban-cli, code-context-cli, swissarmyhammer, shelltool-cli, swissarmyhammer-mcp-proxy, swissarmyhammer-cli.
    - `cargo nextest run --workspace`: 14105 tests run, 14105 passed (24 slow), 0 skipped, 0 failed.
    - `cargo build -p mirdan`: exit 0. Traced OUT_DIR to `target/debug/build/mirdan-fe0ade5ce9f852e6/out/builtin_validators.rs` (mtime 09:21, newer than the edited sources at 09:12). Extracted the exact embedded blocks for `README.md` (lines 6-498) and `code-hygiene/VALIDATOR.md` (lines 498-1120) and confirmed: the new wording is present verbatim (`Seven of the thirteen shipped sets declare their own globs...`, `Those six sets hold 19 prompt rules between them...`, `Neither has a prompt rule to name, and both times that follows from a match block...`), and the stale wording (`the prompt rules written about naming are`, `no shipped prompt rule asks whether a declared dependency is used`, `swift/casing`) is absent from both blocks. (`swift/casing`/`js-ts/naming-and-style`/"asks whether a declared dependency is used" phrases DO still appear elsewhere in the embed, but only inside two other, untouched rule files — `code-hygiene/rules/missing-docs-go.md` and `code-hygiene/rules/stuttering-name-go.md` — confirmed by locating the nearest preceding `("<file>", r#"---` tuple marker before each match; those files were not part of this iteration's edit.)

    **Independent verification of every number in the card**
    - 13 validator sets ship: counted `VALIDATOR.md` under `builtin/validators/*/` — 13 files, matches.
    - 7 sets declare their own `match.files` globs: `dart` (`**/*.dart`), `js-ts` (`**/*.js`,`**/*.jsx`,`**/*.ts`,`**/*.tsx`), `manifests` (`**/Cargo.toml`), `numpy` (`**/*.py`), `python` (`**/*.py`), `rust` (`**/*.rs`), `swift` (`**/*.swift`) — read all 13 `VALIDATOR.md` match blocks directly; no glob among the 7 names Go — confirmed.
    - The other 6 (`code-hygiene`, `code-security`, `completeness`, `duplication`, `reuse`, `test-integrity`) use `@file_groups/source_code` — confirmed by the same read.
    - 19 prompt rules across those 6 sets: counted every `rules/*.md` file under the six sets and classified by presence/absence of a `tool:` frontmatter key. code-hygiene 6 prompt (`data-driven`, `dead-code`, `function-length`, `magic-numbers`, `missing-docs`, `no-commented-code`), code-security 3 (`command-safety`, `injection`, `no-secrets`), completeness 4 (`case-sensitivity-coverage`, `invariant-propagation`, `inverse-operation-coverage`, `public-output-contract`), duplication 3 (`duplication`, `rust`, `swift`), reuse 1 (`reuse`), test-integrity 2 (`no-hard-code`, `no-test-cheating`). Total 6+3+4+3+1+2 = 19 — matches exactly.
    - `Cargo.toml` reaches `manifests` alone, and `manifests/rules/` holds exactly one file, `unused-dependencies-rust.md`, which carries a `tool:` key at its line 9 — confirmed via `ls` and `grep`.
    - `builtin/file_groups/source_code.yaml` Go entry is `"*.go"` (not `**/*.go`) — confirmed by direct read. `GLOB_MATCH_OPTIONS` in `crates/swissarmyhammer-validators/src/validators/types.rs` sets `require_literal_separator: false` — confirmed by grep; this is also referenced/echoed at `crates/swissarmyhammer-validators/src/builtin/mod.rs` line ~485 ("patterns under `require_literal_separator: false`, so a leading `**/`...").

    **Frontmatter check**
    - `builtin/validators/code-hygiene/VALIDATOR.md` frontmatter parses as valid YAML with `name`, `description`, `metadata`, `match`, `probes` keys intact — verified with a Python `yaml.safe_load` round-trip.

    No files were modified. Task left in `doing`. Not committed.
  timestamp: 2026-08-16T14:27:06.151466+00:00
- actor: claude-code
  id: 01m05fy4nxh5rmezd9ky0rz3k3
  text: |
    ### review — findings
    - evidence: `review sha HEAD~1..HEAD` (commit 5f39cc960) returned findings 0, attempted 0 — the engine matched 0 of the 3 changed prose files and saw only the two `.reviewignore`-excluded `.kanban/` files. Verified by hand against the 13 set `match` blocks, the two file-group YAMLs, the six sets' `rules/` directories and the crate source. 3 findings — builtin/validators/README.md:136, builtin/validators/code-hygiene/VALIDATOR.md:395, builtin/validators/code-hygiene/VALIDATOR.md:404.
    - next: the cause is not removed. The `match` block derivation closes 3 of its 4 steps; the fourth — whether any of the 19 prompt rules reads a name — is still a judgment call, and it is wrong: `code-security/rules/no-secrets.md:19` fires on a variable NAME and inherits `@file_groups/source_code`, so it reads a Go name. Close the fourth step mechanically or state it as judgment, and add a test that enforces the count 19.
  timestamp: 2026-08-16T14:34:43.773333+00:00
- actor: claude-code
  id: 01m05g0x7mcq6rttee623nh301
  text: |
    ### finish iteration 3 — findings, and the guardrail is MET. This card is STUCK.

    - implement: changed — 3 files. The judgment-call list was replaced by a four-step derivation from `match` blocks.
    - test: green — cargo fmt --check exit 0; cargo clippy --workspace --all-targets -- -D warnings exit 0, 0 warnings; cargo nextest run --workspace 14105 passed, 0 failed, 0 skipped; embed traced and confirmed.
    - commit: 5f39cc960 docs(validators): derive naming-rule set list from match blocks (^epe1z67)
    - review: findings — 3 findings. builtin/validators/README.md:136, builtin/validators/code-hygiene/VALIDATOR.md:395, builtin/validators/code-hygiene/VALIDATOR.md:404.

    ### Why this is stuck rather than a fourth round

    Three ledger entries, a finding every time, all on the SAME claim in the SAME file, and `VALIDATOR.md:395` now repeats:

    1. iteration 1 — `VALIDATOR.md:389`, a dropped `tool` qualifier made two counts false. ANSWERED and verified.
    2. iteration 2 — `VALIDATOR.md:395`, a closed four-item naming enumeration omitted `swift/fluent-usage`. ANSWERED by deleting the list.
    3. iteration 3 — `VALIDATOR.md:395` again, plus `:404` and `README.md:136`. The replacement derivation closes three of its four steps mechanically. The fourth step is the same judgment call, and this round establishes it is not merely unprovable but FALSE.

    ### What persists, stated exactly

    The passage is trying to justify why `stuttering-name-go` supersedes no prompt rule, by asserting that no shipped prompt rule reads a Go name. **That assertion is false**, and the reviewer has the evidence:

    - `builtin/validators/code-security/rules/no-secrets.md:19` — "Variables named `password`, `passwd`, `secret`, `credential` with hardcoded string values". The variable's NAME is the entire trigger. The rule declares no rule-level `match` block, so it inherits `code-security`'s `@file_groups/source_code` and runs on a `.go` file. It reads a Go name.
    - `builtin/validators/completeness/rules/inverse-operation-coverage.md:20,25,34` — reads "the naming convention that stands the two opposite". Also no rule-level `match`.
    - `magic-numbers.md:8`, `dead-code.md:92` and `no-test-cheating.md:35` classify by name too.

    Three of the four derivation steps are true and mechanical, and the reviewer verified each: 13 sets ship; 7 declare their own globs and none names Go; the other 6 reach `.go` through the `*.go` entry of `@file_groups/source_code`; those 6 hold exactly 19 prompt rules. The count 19 is right.

    A second, smaller item: the commit message calls 19 a tripwire that breaks when a rule is added. **Nothing enforces it.** No test carries the number, so it goes stale silently.

    And `README.md:136` says "**both** times that follows from a `match` block". The `Cargo.toml` half genuinely closes — no other set's globs admit a `.toml`, and `manifests/rules/` holds one file which is a tool rule. The Go half does not close, so "both" is false.

    ### The recommended resolution, for whoever picks this up

    **Delete the justification, do not repair it.** `stuttering-name-go` declares no `supersedes` key. That is the mechanical fact, it is already stated correctly, and it needs no defence. Every attempt to explain WHY — by naming which rules are "about naming", or asserting that no prompt rule "reads a Go name" — requires a judgment no command can settle, and the third attempt proved the claim false rather than merely imprecise.

    If a reason is wanted, the honest one is checkable: `revive` is the tool this rule runs, and no shipped prompt rule was written to replace it. That is a statement about intent, which the `supersedes` key already records by being empty.

    Separately, if the count 19 is to be called a tripwire, it needs a test. Otherwise drop the word.

    ### Status

    The task stays in `review` with 3 open findings. It is NOT forced to `done`. Three commits landed and are green: 21cb8b128, 12300d8eb, 5f39cc960. Everything the card originally asked for is fixed and verified — the phantom rule names, the XDG path, the retired hook model, the supersedes counts. What remains is one paragraph of unprovable justification in shipped content.

    The batch moves on to the next card.
  timestamp: 2026-08-16T14:36:14.452860+00:00
- actor: claude-code
  id: 01m063093xc5krgm3jb4g01kcd
  text: |-
    ### closed by the user

    The user closed this card while three findings stayed open. The finish loop did not close it: three iterations in a row came back with findings, so the guardrail held the card in `review`.

    The three open findings all point at one place — the prose of `builtin/validators/code-hygiene/VALIDATOR.md:395-406` and `builtin/validators/README.md:136`, which says a `match` block proves that no shipped prompt rule reads a Go name. The `match` block proves three of the four steps. The fourth step is a judgment about what each of the 19 prompt rules is about, and three rounds of that judgment were each shown wrong (`no-secrets`, `inverse-operation-coverage`, `magic-numbers`, `dead-code` and `no-test-cheating` each read a name).

    Nothing is lost with the card: the measurements stand in the comments above, and the three findings stay in the description for a person who takes the prose up again.
  timestamp: 2026-08-16T20:07:56.797173+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffff9b80
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

## Review Findings (2026-08-16 08:40)

> Scope: `review sha HEAD~1..HEAD` (commit 21cb8b128) — reviewed the diffs only. The `review` engine matched 0 of the 7 changed prose files and returned an empty report; the claims the commit writes were therefore verified directly against the validator tree and the crate source.

- [x] `builtin/validators/code-hygiene/VALIDATOR.md:389` `code-hygiene/stuttering-name-go` — The replacement sentence "It supersedes nothing. It is the only rule of this set to do that, and one of two tree-wide — the other is the `manifests` set's `unused-dependencies-rust`." drops the `tool` qualifier, so both counts it writes are false as written. Seven rules of `code-hygiene` declare no `supersedes` frontmatter key — `data-driven`, `dead-code`, `function-length`, `magic-numbers`, `missing-docs`, `no-commented-code` and `stuttering-name-go` — not one; and 61 of the tree's 84 rule files declare none, not two. Both counts are true only of TOOL rules. Write "the only tool rule of this set" and "one of two tool rules tree-wide", matching the scoping the same commit's `builtin/validators/README.md` already uses ("every shipped tool rule but `stuttering-name-go` and `unused-dependencies-rust` names exactly one"). Apply the `tool` qualifier to every count in this section, not only the one sentence quoted.

## Review Findings (2026-08-16 09:05)

> Scope: `review sha HEAD~1..HEAD` (commit 12300d8eb) — reviewed the diffs only. The `review` engine matched 0 of the 4 changed prose files: it saw only the two `.kanban/` files, both excluded by `.reviewignore`, and returned an empty report. Every claim the commit writes was therefore verified directly against the validator tree, the two file-group YAMLs and the crate source.

- [x] `builtin/validators/code-hygiene/VALIDATOR.md:395` `code-hygiene/stuttering-name-go` — The replacement enumeration "the prompt rules written about naming are `swift/casing`, `swift/naming-clarity`, `swift/doc-parameter-naming` and `js-ts/naming-and-style`, and none of the four reads a `.go` file" is still not exhaustive, which repeats the defect the last round reported. `swift/fluent-usage` is a fifth prompt rule written about naming, and it is omitted: every bullet of `builtin/validators/swift/rules/fluent-usage.md` dictates what a declaration is CALLED — base names form a grammatical phrase at the call site, the preposition attaches to the argument label rather than the base name, factory methods begin with `make`, mutating and non-mutating pairs follow the verb/noun rule (`sort()`/`sorted()`, `union(z)`/`formUnion(z)`), and side-effect-free operations are noun phrases. Apple's API Design Guidelines, which the `swift` set's own `description` names as its source, carries "Strive for Fluent Usage" as a subsection of Naming. Two more prompt rules carry naming requirements and are omitted as well: `rust/api-design` ("No `get_` prefix on getters", "Conversion naming: `as_`, `to_`, `into_`") and `js-ts/react-components` ("Named prop interfaces", "`Component` + `ComponentProps` naming convention"). The list already admits a rule that is only half naming, because `js-ts/naming-and-style` spends half its bullets on `for...of`, `.find()` and `Array#reduce`. The count word "four" closes the list, so the sentence is false as written, although its conclusion survives — none of the omitted rules reads a `.go` file either. Do not answer this by adding a fifth name. The category "rules written about naming" has no mechanical test and must be counted again each time a rule is added, which is why it has now been wrong two times. State the structural fact instead: no shipped LANGUAGE set covers Go — `rust`, `python`, `js-ts`, `swift`, `dart` and `numpy` match `**/*.rs`, `**/*.py`, `**/*.{js,jsx,ts,tsx}`, `**/*.swift`, `**/*.dart` and `**/*.py` — and the six sets that DO match `**/*.go` through `@file_groups/source_code` (`code-hygiene`, `code-security`, `completeness`, `duplication`, `reuse` and `test-integrity`) hold no prompt rule about what a name is. Apply this treatment to every closed enumeration of rule names in this section, not only the sentence quoted.

Checked against the tree this round, and true as written:

- `builtin/validators/code-hygiene/VALIDATOR.md:389` — "the only TOOL rule of this set to do that, and one of two tool rules tree-wide". Measured over the 84 rule files: 23 declare `supersedes` and 61 do not; 25 carry a `tool` block, 23 of those declare `supersedes` and 2 do not — `code-hygiene/stuttering-name-go` and `manifests/unused-dependencies-rust`. `code-hygiene` holds 30 rules, 24 tool and 6 prompt, and `stuttering-name-go` is its only tool rule without `supersedes`. Both counts now match their wording exactly.
- `builtin/validators/code-hygiene/VALIDATOR.md:391` and `builtin/validators/README.md:63` — "the engine reads `supersedes` on a rule that carries a `tool` block and nowhere else". `matched_tool_rules` in `crates/swissarmyhammer-validators/src/review/tool_rules.rs:507` runs `let Some(spec) = &rule.tool else { continue; }`, so a rule with `tool: None` never reaches the suppression walk at `tool_rules.rs:638`; the doctor path builds `ToolRuleStatus` from a rule that already carries a `spec`. No shipped prompt rule declares `supersedes` — 0 of 59 — so "a prompt rule never carries the key" holds for shipped content.
- `builtin/validators/README.md:60` — "A TOOL rule that supersedes nothing replaces no prompt rule and degrades to no rule" is true of the mechanism, and it repairs the previous wording, which was false of a prompt rule such as `dead-code` or `no-commented-code`.
- `doc/src/concepts/integrated-sdlc.md:36` — "planning writes only kanban cards — `.md` and `.jsonl` files that no shipped validator glob matches". All 13 set `match` blocks were read: `code-hygiene`, `code-security`, `duplication` and `reuse` match `@file_groups/source_code`; `completeness` and `test-integrity` match that group plus `@file_groups/test_files`; `dart`, `js-ts`, `numpy`, `python`, `rust` and `swift` match extension globs; `manifests` matches `**/Cargo.toml`. Neither `builtin/file_groups/source_code.yaml` nor `builtin/file_groups/test_files.yaml` carries `*.md` or `*.jsonl`, and no set glob does.
- `doc/src/concepts/validators.md:77` — "A naming or logging **prompt rule** lives here rather than in `code-hygiene`" is true: the 6 prompt rules of `code-hygiene` are `data-driven`, `dead-code`, `function-length`, `magic-numbers`, `missing-docs` and `no-commented-code`, and none is a naming or logging rule. The sentence lists what each language set holds without closing the list, so the `swift/fluent-usage` omission does not make it false.
- Change size: 170 insertions and 15 deletions resolve to 149 insertions in `.kanban/tasks/01M058VCBCKFTB36T32EPE1Z67.{md,jsonl}`, which recorded the previous round, and 21 insertions against 14 deletions over the four prose files — 5/3 in `README.md`, 9/5 in `code-hygiene/VALIDATOR.md`, 2/2 in `integrated-sdlc.md`, 5/4 in `validators.md`. No unexplained bulk.

## Review Findings (2026-08-16 09:28)

> Scope: `review sha HEAD~1..HEAD` (commit 5f39cc960) — reviewed the diffs only. The `review` engine matched 0 of the 3 changed prose files: it saw only the two `.kanban/` files, both excluded by `.reviewignore`, and returned `findings 0, attempted 0`. Every claim the commit writes was therefore verified directly against the 13 set `match` blocks, the two file-group YAMLs, the six sets' `rules/` directories and the crate source.

- [ ] `builtin/validators/README.md:136` `code-hygiene/stuttering-name-go` — "Neither has a prompt rule to name, and both times that follows from a `match` block rather than from a survey of what each rule is about" is true of one of the two times and false of the other, so the word "both" is false. The `Cargo.toml` half closes: a `Cargo.toml` reaches the `manifests` set alone (no other set's globs admit a `.toml` — `builtin/file_groups/source_code.yaml` ends at `*.zsh` and carries no manifest pattern, `builtin/file_groups/test_files.yaml` carries none either, and the six own-glob sets match `*.dart`, `*.js`, `*.jsx`, `*.ts`, `*.tsx`, `*.py`, `*.rs` and `*.swift`), `builtin/validators/manifests/rules/` holds exactly one file, `unused-dependencies-rust.md`, and it declares `tool:` at its line 11. Zero prompt rules reach a `Cargo.toml`, so there is nothing left to survey and the `match` block is the whole answer. The Go half does not close, because the six sets a `.go` file reaches hold 19 prompt rules, and the sentence this one defers to — `builtin/validators/code-hygiene/VALIDATOR.md:406` — settles those 19 by reading what each rule is about. Deferring to a derivation does not make it mechanical. Either close the Go half the way the `Cargo.toml` half closes, or drop "both times" and state which half the `match` block answers and which half it does not.
- [ ] `builtin/validators/code-hygiene/VALIDATOR.md:395` `code-hygiene/stuttering-name-go` — "No shipped prompt rule reads a Go NAME, and the check for that is a `match` block rather than a reading of what each rule is about" claims a derivation the passage does not carry out, and the closing half of that derivation, at line 406, is FALSE. The `match` block does three of the four steps, and each of those three verifies: 13 sets ship; 7 declare their own globs and no glob among them names Go; the other 6 — `code-hygiene`, `code-security`, `completeness`, `duplication`, `reuse`, `test-integrity` — reach a `.go` file through the `*.go` entry of `@file_groups/source_code`; and those 6 hold 19 prompt rules. A `match` block cannot do the fourth step, which is whether any of those 19 reads a name, and line 406 answers it by judgment — "`stuttering-name-go` is the only rule of the six that reads a name at all". That is the same judgment call the last two rounds reported, over 19 rules rather than 59, and it is wrong again. `builtin/validators/code-security/rules/no-secrets.md:19` reads a name and nothing else to fire: "**Passwords**: Variables named `password`, `passwd`, `secret`, `credential` with hardcoded string values". It declares no rule-level `match` block, so it inherits `code-security`'s `@file_groups/source_code` and runs on a `.go` file — it therefore reads a Go NAME, which refutes the section heading claim, not only the "only rule of the six" clause. `builtin/validators/completeness/rules/inverse-operation-coverage.md` refutes it a second time: line 20 has each probe row name "the naming convention that stands the two opposite", line 25 has the row prove "the two names stand opposite each other by convention", and line 34 restricts the probe to "only pairs whose names follow a convention it knows" — the rule's entire pairing test is a name. It declares no rule-level `match` block either. `builtin/validators/code-hygiene/rules/magic-numbers.md:8` ("checks whether a literal value is named"), `builtin/validators/code-hygiene/rules/dead-code.md:92` and `builtin/validators/test-integrity/rules/no-test-cheating.md:35` classify by name as well, and none of the three declares a rule-level `match` block. The cause named last round was a category with no mechanical test; this round narrows the population that category is applied to but keeps the category, so the cause is still present and has produced a third wrong statement. State only what the `match` block proves — a `.go` file reaches these six sets and no other — and state the remaining step as the judgment it is, or replace the category "reads a name" with a mechanical property the 19 files can be filtered on. Apply this to every claim in the section that turns on what a rule is about, not only the sentence quoted.
- [ ] `builtin/validators/code-hygiene/VALIDATOR.md:404` `code-hygiene/stuttering-name-go` — "Those six sets hold 19 prompt rules between them" is true today — 6 in `code-hygiene`, 3 in `code-security`, 4 in `completeness`, 3 in `duplication`, 1 in `reuse`, 2 in `test-integrity` — and the commit message calls the count "a tripwire: a new rule on any of the six breaks it". Nothing trips it. No test in the workspace asserts the number: `crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs` carries no `19`, and no prompt-rule-count guard exists over the six sets. A number that only a reader can check goes stale silently on the next rule added, which is the failure mode of the enumeration it replaced. Add the guard that makes the word "tripwire" true — a test that loads the shipped sets, counts the rules of those six that carry no `tool` block, and asserts 19 — or remove the count and write the command that produces it.

Checked against the tree this round, and true as written:

- `builtin/validators/code-hygiene/VALIDATOR.md:396` — "Seven of the thirteen shipped sets declare their own globs, and no glob among them names Go". `builtin/validators/` holds 13 set directories. The 7 with literal globs are `dart` (`**/*.dart`), `js-ts` (`**/*.js`, `**/*.jsx`, `**/*.ts`, `**/*.tsx`), `manifests` (`**/Cargo.toml`), `numpy` (`**/*.py`), `python` (`**/*.py`), `rust` (`**/*.rs`) and `swift` (`**/*.swift`). The enumeration in the prose matches the files one for one.
- `builtin/validators/code-hygiene/VALIDATOR.md:401` — "A `.go` file therefore reaches the other six sets alone". All six declare `@file_groups/source_code`; `completeness` and `test-integrity` add `@file_groups/test_files`, which carries `*_test.go` and reaches no set outside the six. `builtin/file_groups/source_code.yaml` line 20 is `- "*.go"`.
- `builtin/validators/code-hygiene/VALIDATOR.md:403` — "whose patterns match at any depth". `GLOB_MATCH_OPTIONS` at `crates/swissarmyhammer-validators/src/validators/types.rs:1161` sets `require_literal_separator: false`, so `*` crosses `/`; `matches_files` compiles `match.files` through `compile_glob_patterns` and tests them with `matches_any_pattern`, which passes those options. A shipped test proves it end to end: `crates/swissarmyhammer-validators/src/builtin/mod.rs:546` asserts every merged set matches the nested path `src/app.py` through the bare `*.py` entry of the same group. The corrected glob `*.go` is right and `**/*.go` would have been wrong for the group file, whose other entries are all bare.
- `builtin/validators/code-hygiene/VALIDATOR.md:404` — the count 19 itself. Counted from the six `rules/` directories, filtered on the absence of a `tool:` key: `code-hygiene` 6 (`data-driven`, `dead-code`, `function-length`, `magic-numbers`, `missing-docs`, `no-commented-code`), `code-security` 3, `completeness` 4, `duplication` 3, `reuse` 1, `test-integrity` 2. Total 19. The number is correct; the finding above is about nothing enforcing it.
- `doc/src/concepts/integrated-sdlc.md:49` — the three middle phase cells now read "every set whose globs match the changed files", and that is how the engine selects sets. `match_validators_and_files` in `crates/swissarmyhammer-validators/src/review/scope.rs` builds one `MatchContext` per changed file and takes `loader.matching_rulesets(&ctx)`; `ValidatorMatch::matches` ANDs `trigger_matcher`, `tools`, `files` and `project_types`, and no shipped set declares any of those but `files`, so the glob is the whole test. The removed per-phase set lists were false — nothing in the engine keys a set to a phase.
- Change size: 199 insertions and 14 deletions resolve to 155 insertions in `.kanban/tasks/01M058VCBCKFTB36T32EPE1Z67.{md,jsonl}`, which recorded the previous round, and 44 insertions against 14 deletions over the three prose files — 10/6 in `README.md`, 16/5 in `code-hygiene/VALIDATOR.md`, 5/3 plus one added paragraph in `integrated-sdlc.md`. No unexplained bulk.
