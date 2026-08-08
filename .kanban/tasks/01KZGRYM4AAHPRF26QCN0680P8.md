---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzhk8dbfwedb86qf4y16zt0w
  text: |-
    ### Research — where the pieces live

    **Grammar roster.** There are TWO. `swissarmyhammer-treesitter/src/language.rs` (30 languages, drives indexing and `query ast`) and `swissarmyhammer-sem/src/parser/plugins/code/languages.rs` (16 languages). The sem roster is the one with the documented single entry point — `parse_code(path, source) -> Option<ParsedCode>`, whose doc says "every consumer that needs a parse calls it instead of building a `tree_sitter::Parser` and picking a language itself". The card names sem, and the per-language spec-table shape (`ComplexitySpec`, `TestCensusSpec`) already lives beside it. The extractor goes there.

    Dependency edges that decide placement: `swissarmyhammer-treesitter` DEPENDS ON `swissarmyhammer-code-context`, so code-context cannot use the treesitter roster. `swissarmyhammer-sem` has no swissarmyhammer dependencies at all, so code-context may depend on it. Placement: extractor in sem, op wrapper in `code-context/src/ops/`, handler in `swissarmyhammer-tools`.

    **Op wiring** (12 touch points): ops module + `ops/mod.rs` + `lib.rs` re-export; in `swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` the `use` list, the op unit struct + `ParamMeta` array + `impl Operation`, the `Lazy` static, the `CODE_CONTEXT_OPERATIONS` vec, the `match` arm, BOTH "Valid operations:" literals, the module doc list, and two tests asserting `== 24`.

    **CLI shape.** `sah tool code_context <noun> <verb>`. `sah tool` prints YAML for a JSON result and RAW TEXT for a non-JSON result (`mcp_integration.rs::format_success_response`). So the op returns plain text — one `path:line: message` line per block — and the rule needs no `jq`.

    **Parameter-name collision hazard, measured.** `sah tool code_context ast query --query X --language rust` fails with "missing required parameter 'language'". Cause: the CLI extracts args from the FLAT merged `properties` of the full schema, and `grep code` declares `language` as an array while `query ast` declares it a string; the array shape wins, so the tool receives `["rust"]` and `as_str()` returns None. Pre-existing bug, not this card. Consequence for this card: the new op reuses the existing array-typed name `files`, never a new name that collides at a different type.

    **Binary resolution.** `run_shell` (`validators/src/doctor.rs`) is the ONE runner for every tool-rule script and sets no env today. It gets `SAH_BIN`, resolved as env override, then `current_exe()` when its file stem is `sah`, then the bare name. Under `cargo nextest` `current_exe()` is a test binary, which is why the stem check is there.

    **Fixture-pair mechanism.** `find_fixture` matches the prefix `<rule>.fail.` with `read_dir().find(...)`, so a rule has exactly ONE fail fixture and ONE pass fixture, and shipping three would make doctor pick nondeterministically. Per-language coverage therefore lands in sem unit tests plus a production-path acceptance test over a Rust + Python + TypeScript probe repository, which is strictly more than three fixture pairs would prove.
  timestamp: 2026-08-08T21:07:57.423323+00:00
- actor: claude-code
  id: 01kzhmejd88aap04yycmpma9td
  text: |-
    ### Extractor landed, and the thresholds come from measurement

    `crates/swissarmyhammer-sem/src/parser/plugins/code/commented_code.rs`. Eleven languages get a row: rust, python, typescript, tsx, javascript, go, java, c, cpp, csharp, swift. Five of the sem roster's sixteen get none — `bash`, `ruby` and `elixir` accept a paren-less call, so English parses as a command with arguments; `php` needs an opening tag its comment text never carries; `fortran` has no delimiter convention that separates documentation from a disabled line.

    **Node kinds were read off the grammars, never guessed.** A probe dumped every comment node kind for all sixteen languages. Rust is the ONE grammar that gives a doc comment its own node — `outer_doc_comment_marker` / `inner_doc_comment_marker` as a CHILD of `line_comment` / `block_comment`. Java and Swift split comments into two kinds (`line_comment`/`block_comment`, `comment`/`multiline_comment`); the other eight spell every comment `comment`. So the doc exclusion is two structural tests: the grammar's own marker node where it exists, then the comment's own opening delimiter (`///`, `/**`, `//!`, `/*!`) where it does not. Neither reads prose.

    **Measured corpus**: this workspace (1610 files measured) plus `psf/requests` (37), `axios/axios` (234), `BurntSushi/ripgrep` (110) and `gohugoio/hugo` (934) — 2925 files. 1949 comment blocks clear the 5-line gate. Every block whose error ratio was under 0.31 was read by hand.

    Three populations came out of that reading:

    | What the block is | Lowest ratio | Highest ratio |
    |---|---|---|
    | commented-out code | 0.000 | 0.035 |
    | standardized metadata | 0.110 | 0.137 |
    | prose | 0.173 | 0.999 |

    The binding pair is 0.035 and 0.110, so the gate is **0.07** — twice the code figure, two thirds of the metadata one. The first guess of 0.15 was wrong: it admitted all three PEP 723 headers in `crates/ane-embedding/convert`.

    **Final counts at 0.07**: 0 findings on this workspace, 0 on requests, 0 on axios, 1 on ripgrep, 2 on hugo. All three hand-checked and real — ripgrep's 22 disabled `println!` lines in `crates/index/src/literal.rs`, hugo's `/*if !c.skipTidy ...*/` in `modules/collect.go`, and six disabled calls in `htmltemplate/exec_test.go`.

    **Two false-positive classes the first cut had, both fixed structurally:**

    1. `gofmt` aligns a run of trailing comments into one column, so a same-column test cannot tell six annotations from a block — `hugo/common/predicate/predicate_test.go` re-parsed six `// true || false && ...` tails as six clean Go statements. The column test was replaced by an own-line test: a comment with live code to its left annotates that line.
    2. tree-sitter-go reads a bare word as `(expression_statement (identifier))`, so `hugo/tpl/tplimpl/render_hook_integration_test.go`'s seven-word `// Hooks:` list re-parsed with no error node at all. An item now counts only when it holds more than a bare name.

    **Three behaviors were REMOVED because no realistic input distinguished them** — each was a mutation that stayed green: the one-space strip after a comment marker, the same-column grouping test, and a `# ///` PEP 723 delimiter list (the error gate already rejects that shape at 0.110 against a 0.07 gate).

    **RED verified 14 ways.** Every gate, every delimiter table and every strip was broken on purpose and the failing test recorded: line gate to 1 and to 6, item gate to 1 and to 3, error ratio to 1.01 and to strict, Rust doc-marker nodes removed, C-family doc openers removed, own-line filter removed, bare-name substance test removed, block-continuation strip removed, line-opener strip removed, block delimiters emptied. 16 tests green.
  timestamp: 2026-08-08T21:28:47.784489+00:00
- actor: claude-code
  id: 01kzhp3btvpvtahhwvrdkbpcqa
  text: |-
    ### The op, the rule, and the acceptance bar

    **The op.** `find commented_code` — `sah tool code_context commented_code find --files <path> ...`. Twelve wiring points, all of them touched: the ops module, `ops/mod.rs`, the `lib.rs` re-export, the tools crate's `use` list, the op unit struct with its `ParamMeta` array and `impl Operation`, the `Lazy` static, `CODE_CONTEXT_OPERATIONS`, the dispatch arm, BOTH "Valid operations:" literals, the module doc list, the schema example, and the three counts that assert an op total (24 → 25, and the example count 14 → 15).

    It opens no workspace. The verdict is a parse of the files named, so the op answers without the code-context index and runs in a scratch directory that holds no `.code-context` database — which is exactly where the doctor's fixtures live.

    **The result is plain text, and that is deliberate.** `sah tool` renders a JSON result as YAML (`mcp_integration.rs::format_success_response`), and the tool-rule stdout contract cannot read YAML. The op returns the finding lines and nothing else, so the rule's `run` needs no `jq`. Verified with `cat -e`: `demo/probe.rs:3: commented-out code (7 lines parse as rust)$` — one line, no banner, no leading newline, exit 0. Empty stdout on a clean file.

    **The `files` parameter name was forced by measurement.** The CLI extracts arguments from the FLAT merged `properties` of the full schema, so two ops declaring one name at two types collide — `query ast --language` is already broken that way. `files` is array-typed in `grep code` and `query ast`, so reusing it is safe; a new name would not have been.

    **`SAH_BIN`.** `run_shell` in `validators/src/doctor.rs` is the ONE runner for every tool-rule script, and it now exports `SAH_BIN` to all of them. Resolution: an existing `SAH_BIN`, then `current_exe()` when its file stem is `sah`, then the bare name. The stem check is why a test run does not hand a script its own test binary. Two tests pin it, and both go RED when the export or the override is removed.

    **The acceptance bar is checked, not asserted.** `prompt_rules_for` was extracted out of `plan_fan_out` — it IS the fan-out planner's filter, the list of rules an agent is handed for a file. The acceptance test reads it for each matched file and asserts `no-commented-code` is absent, so no task can carry the rule and no agent can ever read it. Beside it, `ToolReport::attempted() == 1` for this rule: one process replaced three agent turns.

    **Where the acceptance test lives, and why not beside its siblings.** `apps/swissarmyhammer-cli/tests/commented_code_tool_rule.rs`. This rule's tool IS sah, and `cargo` defines `CARGO_BIN_EXE_sah` only for that package's own integration tests — it is the one place a test can name the binary it just built. Adding a row to `SHIPPED_*_RULES` in `tool_rules.rs` instead would resolve `SAH_BIN` to whatever `sah` sits on `PATH`, which on any machine with an older copy reports the rule missing and trips that helper's `exercised > 0` guard. The new test does everything the sibling acceptance tests do — a healthy plan proves the fixture pair passed, the suppression is asserted, and the finding comes from the real script — over three languages rather than one.

    **RED verified 6 more ways** on the shipped rule, by breaking the rule file itself: `supersedes` removed, `"$SAH_BIN"` replaced by a bare `sah` (this broke BOTH tests, which is the proof the variable is load-bearing — the PATH copy is stale), `.py` dropped from the match, `.ts` dropped from the match, an extension dropped from the match, and an extension added to it.

    **Doctor rows, verified in a scratch project with HOME pointed at a scratch directory** (nothing under the real home was read or written):

    - healthy: `✓ Validator Tool Rule · code-hygiene/no-commented-code-parsed — tool present (swissarmyhammer 0.17.0 (d56e330399cb)); fixtures pass`
    - degraded, with `SAH_BIN=/nonexistent/sah`: `⚠ ... tool missing: ...; prompt rule 'no-commented-code' runs instead` and `Fix: Install the tool: put the running sah binary on PATH, or set SAH_BIN to its path`

    **The ruff ERA cross-check was RUN, not cited.** `ruff 0.14.5`, `--isolated --no-cache --select ERA001`. The two verdicts agree on every shape where both have an opinion — a commented-out function (ERA 3, this rule 1), a six-line TODO (both clean), a docstring example (both clean), a PEP 723 header (both clean). The one difference is the two-line snippet, which ERA reports and this rule does not, and that is the line gate rather than a disagreement about the text. Over this workspace's Python, ERA reports 1 finding and, like this rule, says nothing about the three PEP 723 headers. The table is in the rule body.

    **Fixtures.** `find_fixture` matches the prefix `<rule>.fail.` with `read_dir().find(...)`, so a rule has exactly one pair and shipping three would make the doctor pick nondeterministically. The shipped pair is Rust: the fail fixture is a seven-line commented-out function, and the pass fixture carries all three shapes the card lists — a doc-comment example, a TODO written as prose, and a two-line snippet. Rust, Python and TypeScript coverage is delivered by the production-path acceptance test instead, which is strictly more than three fixture pairs would prove.

    **No finding on its own diff**: the rule reports nothing over the fifteen source files this change touches.
  timestamp: 2026-08-08T21:57:37.755199+00:00
- actor: claude-code
  id: 01kzhpa18ry03m1v53hwge57aj
  text: |-
    ### implement — changed
    - evidence: 20 files — 6 new (`crates/swissarmyhammer-sem/src/parser/plugins/code/commented_code.rs`, `crates/swissarmyhammer-code-context/src/ops/find_commented_code.rs`, `apps/swissarmyhammer-cli/tests/commented_code_tool_rule.rs`, `builtin/validators/code-hygiene/rules/no-commented-code-parsed.md`, and the rule's two `.tmpl` fixtures), 14 modified. `cargo nextest run --workspace` 13898 passed / 0 failed / 0 skipped; `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean. RED verified 21 ways — 14 mutations of the extractor, 1 of `end_row`, 2 of the `SAH_BIN` export, 4 of the shipped rule file, plus 2 of the extension guard.
    - next: `/review`
  timestamp: 2026-08-08T22:01:16.312906+00:00
position_column: doing
position_ordinal: '8480'
title: 'no-commented-code: ruff ERA tool rule for Python + a comment-reparse tree-sitter probe'
---
## STANDING ORDER

This card is a decision, not a proposal. Your job is to implement it exactly.
- Do NOT downgrade this to a probe that feeds a prompt rule. The parse verdict decides. Zero LLM calls for this rule on matched files is the acceptance bar.
- Do NOT keep the prompt rule running for matched files. Supersede it.
- Do NOT file a follow-up card in place of doing the work.
- Stop and escalate ONLY when a step is impossible. Report the exact command and its output.

## The work

Correction: the first version of this card made the tree-sitter reparse a probe feeding the prompt rule. That was wrong. The verdict is objective — comment content either parses as code for the file's language or it does not. No LLM reads it.

1. New sah op (in swissarmyhammer-sem / code-context, where the grammar roster lives): for each file argument, extract comment blocks with tree-sitter, strip the comment markers, and re-parse the text with the file's own grammar. A block over 5 lines whose reparse yields 2 or more statements/items with an error-node ratio under a fixed threshold IS commented-out code. Emit one line per block: `path:line: commented-out code (<n> lines parse as <language>)`. Exclude doc-comment node kinds structurally — a documentation example is never a finding, by grammar node kind and not by prose.

2. Tool rule `no-commented-code-parsed` in `code-hygiene/rules/`: files scope, `run: sah <op> "$@"`, `supersedes: no-commented-code`. The match lists the extensions the grammar roster covers, explicitly. A language without a grammar keeps the prompt rule — fallback by match, the designed degradation. Doctor: the tool is sah itself, so `check_command` names the sah binary; no install commands. Resolve the binary the way the engine invokes itself (env or current_exe), never a bare PATH assumption.

3. The exemption contract is structural, never prose: put intentional example code in a doc comment, or keep the block at 5 lines or fewer. State this in the rule body.

4. Drop the separate ruff ERA rule from the earlier plan — one owner per finding, and the reparse op covers Python. Note ERA in the rule body as the cross-check used to validate the Python fixtures.

5. Fixtures: fail fixtures hold a commented-out function of 6+ lines for at least Rust, Python, and TypeScript; pass fixtures hold a doc-comment example, a TODO with prose, and a short 2-line snippet. Unit tests in the sem crate for the extractor per language family. Extend the shipped-rules acceptance test. Acceptance: a review of a file whose only defect is a commented-out block reports it with zero LLM validator calls for that rule.

#tool-validators #objectivity