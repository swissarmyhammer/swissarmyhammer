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
- actor: claude-code
  id: 01kzhrmsvsf8ex4kr97g2fphk3
  text: |
    ### review — findings
    - evidence: 12 findings recorded, from `review sha d56e33039..d861843cf` (engine returned 14 confirmed / 9 refuted / 27 attempted; 1 file skipped over the prompt cap). Locations: `crates/swissarmyhammer-code-context/Cargo.toml:15`, `:28`, `:33`, `:34`; `crates/swissarmyhammer-code-context/src/ops/find_commented_code.rs:48`; `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:1`; `crates/swissarmyhammer-tools/src/mcp/tools/code_context/schema.rs:100`, `:169`; `crates/swissarmyhammer-validators/src/doctor.rs:423`, `:521`, `:682`, `:697`.
    - dropped: 2 duplication findings on `schema.rs:169` and `schema.rs:192` that asked to extract the repeated literal `14` into a named constant inside the pre-existing `mod tests`. The review skill drops every finding whose subject is refactoring test code that already existed.
    - next: `/implement` the 12 checklist items, then re-review. Task stays in `review`.
  timestamp: 2026-08-08T22:42:06.329764+00:00
- actor: claude-code
  id: 01kzhrpmjhq08s9vm4mashfse5
  text: |-
    ### finish iteration 1 — findings
    - implement: changed — 20 files; new commented_code extractor in sem, find_commented_code op in code-context, no-commented-code-parsed rule, SAH_BIN export on the shared shell runner, prompt_rules_for extracted from plan_fan_out. RED verified 21 ways
    - test: green — cargo nextest run --workspace, 13898 passed, 0 failed, 0 skipped; fmt clean; clippy -D warnings clean. Targeted re-run confirmed the shared run_shell change did not break the other 24 shipped tool rules
    - commit: d861843cf — 24 files, 2562 insertions, 28 deletions
    - review: findings — 12 open, on `review sha d56e33039..d861843cf`. Engine: 14 confirmed, 9 refuted, 27 attempted, 0 failed, 1 file skipped; 2 dropped under the pre-existing-test exception

    Findings by file:
    - crates/swissarmyhammer-code-context/Cargo.toml:15, :28, :33, :34
    - crates/swissarmyhammer-code-context/src/ops/find_commented_code.rs:48
    - crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:1
    - crates/swissarmyhammer-tools/src/mcp/tools/code_context/schema.rs:100, :169
    - crates/swissarmyhammer-validators/src/doctor.rs:423, :521, :682, :697

    Three notes for the next iteration:

    The new extractor `commented_code.rs` drew ZERO findings across every validator that read it, and `fleet.rs` drew zero — the `prompt_rules_for` extraction was accepted. Both judgment calls flagged for scrutiny (Rust-only fixture pair, acceptance test in `apps/swissarmyhammer-cli/tests/`) also drew zero.

    `doctor.rs:682` is the one finding specific to this change: `exe.file_stem() == Some(OsStr::new(SAH_BINARY_NAME))` compares case-sensitively, so a Windows `SAH.exe` fails the stem test. The other three doctor findings are reuse against `run_fixture()`, `command_failure_detail()` and `run_shell()` duplicating `prepare_shell_command`.

    `code_context/mod.rs` renders to 520167 bytes against the 262144-byte per-file cap, so NINE validators could not read it at all — code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity. This change put twelve op-wiring touch points in that file and none of them were reviewed. Card ^gsm2fq8 already exists for this exact split.
  timestamp: 2026-08-08T22:43:06.449786+00:00
- actor: claude-code
  id: 01kzhs2wd533796nmv4ctq1efp
  text: |-
    ### Returning from review — research for the 12 findings

    **Cargo.toml (4).** All four are real. `grep` over `crates/swissarmyhammer-code-context/src` for `swissarmyhammer_project_detection`, `model_embedding`, `once_cell` and `include_dir` returns zero hits each. The workspace carries no `[package.metadata.cargo-machete]` table anywhere, so there is no prevailing ignore-list pattern to follow — the rule body names deletion as the other answer, and nothing needs these to compile. Delete them.

    **`find_commented_code.rs:48`.** `&[String]` → `&[&str]`. `findings_in_file` already takes `&str`, so the body is a pass-through. Six call sites, all in this file's own tests plus the op handler in the tools crate.

    **`schema.rs:100` and `:169` — which number is right, measured.** Both. They count different things. `generate_code_context_examples()` really does return 15 entries (I counted them: get symbol, search symbol, list symbols, grep code, search code, find duplicates, query ast, find commented_code, get callgraph, get blastradius, get status, rebuild index, clear status, lsp status, detect projects), so `examples.len() == 15` is correct. `test_operations()` is NOT the production roster — the production roster is `CODE_CONTEXT_OPERATIONS` in `mod.rs`, which holds 25 ops. `test_operations()` is a hand-maintained subset of 14, so the two assertions never contradicted each other and no test was failing. The defect the finding actually names is the cause of that: a second, hand-copied roster in the test module that drifts from the real one every time an op lands. The fix removes the cause — `test_operations()` returns the production roster, and the two counts derive from it instead of restating a literal.

    **`doctor.rs` — the three reuse findings, and where the partner lives.**
    - `:697` `run_shell` vs `prepare_shell_command`. `prepare_shell_command` is `pub(super)` in `swissarmyhammer-tools/src/mcp/tools/shell/process.rs`, and `swissarmyhammer-tools` DEPENDS ON `swissarmyhammer-validators`, so validators cannot import it. The shared core has to move down into `swissarmyhammer-common`, which both already depend on. The two differ only in the interpreter (`bash` vs the platform shell) and in the positional arguments; the program/flag choice, the working directory and the stdio setup are the same decision written twice.
    - `:521` `command_failure_detail`. Two more copies exist: `swissarmyhammer-tools/src/mcp/tools/code_context/doctor.rs` and `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs`, both spelling "stderr when present, exit status otherwise" by hand.
    - `:423` `run_fixture`. There is no symbol named `fixture_replay` in the tree — the finding named the concept, and the concept is real: `run_fixture` (doctor.rs) and `run_tool_script` (`review/tool_rules.rs`) are two implementations of the same five steps — build the args from `spec.scope`, `run_shell`, map a nonzero exit through `command_failure_detail`, `parse_tool_stdout`, then attribute by `normalize_tool_path`.

    **`doctor.rs:682`** is the one finding specific to this change: the `file_stem()` comparison is case-sensitive and Windows executable names are not.

    **`code_context/mod.rs:1`.** Read `^gsm2fq8`. Doing the split here — see the separate comment.
  timestamp: 2026-08-08T22:49:47.685075+00:00
- actor: claude-code
  id: 01kzhv3rjmftcx5zdtm10q6r6k
  text: |-
    ### The `code_context/mod.rs` split is DONE here — `^gsm2fq8` can be closed

    The split was not deferred. `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` was 4890 lines and 187624 bytes; it is now 337 lines and 14297 bytes. The dispatch, the schema wiring, `CodeContextTool` and the registration stayed; everything the dispatch reaches moved to a sibling:

    | module | what it owns | bytes |
    |---|---|---|
    | `ops.rs` | one metadata struct per op, plus the roster and its accessor | 29107 |
    | `indexing.rs` | the tree-sitter pass and the embedding pass | 25727 |
    | `execute.rs` | the handlers backed by the stored index | 17383 |
    | `lsp_ops.rs` | the handlers backed by a live language server | 16992 |
    | `status.rs` | get status, rebuild index, clear status, lsp status | 14214 |
    | `support.rs` | the supervisor, `open_workspace`, the readiness gate, the notice | 8556 |
    | `tests/` (5 files) | the test module, split the same way | 62654 |

    **The cap arithmetic, not a hope.** A rendered line costs its own bytes plus a fixed 22-byte gutter (`{line:>6} | {sha:8} {mark} | `, read off `review/fleet/render.rs`). The old file's deterministic cost was 187624 + 22 × 4890 = **295204** — already over the 262144-byte cap before a single byte of probe evidence, which is how it reached 520167. The largest file now is `ops.rs` at 29107 + 22 × 873 = **48313**, 5.4x under the cap and below `watcher.rs` (61772), the sibling in this same directory that every review has always read. Nothing in the directory is close to the cap.

    **What the split does NOT change.** The public surface is identical: `mod.rs` re-exports `index_discovered_files_async`, every op struct (`pub use ops::*`), and `any_lsp_session` / `lsp_session_for_file` / `open_workspace` / `LSP_SUPERVISOR` for the two crate consumers (`mcp/server.rs`, `tools/diagnostics/mod.rs`) and the five external ones (`code-context-cli`, the integration tests).

    **One thing the split removed rather than moved**, because the move made it unnecessary: `McpTool::operations()` held an `unsafe { std::mem::transmute }` to give the roster a `'static` lifetime. `CODE_CONTEXT_OPERATIONS` is a `static`, so the borrow already IS `'static`; the new `code_context_operations()` accessor returns `CODE_CONTEXT_OPERATIONS.as_slice()` and compiles with no `unsafe` at all.

    **Judgment call, stated plainly.** I did NOT touch the two verbatim-duplicated 600-character "Valid operations: ..." literals in the dispatch, and I did NOT change the dispatch's behaviour in any way. The split was kept mechanical on purpose so the 115 moved tests are a real regression check on it. Those literals are now readable by the nine validators for the first time and may well be raised next round; that is the split working.

    `^gsm2fq8`'s acceptance is "a review that touches any of the new files reviews them (no per-file cap skip), and the workspace stays green". Both hold. Close it.
  timestamp: 2026-08-08T23:25:13.684242+00:00
- actor: claude-code
  id: 01kzhv53dwzvajmmrwwg5qjfwg
  text: |-
    ### The other eleven findings, and the RED ledger

    **Cargo.toml (4).** All four deleted. `cargo machete 0.9.x` on the package reports nothing now. RED verified four ways: each dependency put back one at a time, and machete names exactly that one each time. No `[package.metadata.cargo-machete]` was added — the workspace carries no such table anywhere, and nothing needs these to compile.

    **`find_commented_code` takes `&[&str]`.** The op handler in the tools crate now collects `Vec<&str>` straight out of the JSON array instead of allocating a `String` per path, so the change removed allocations rather than moving them. RED: the signature put back to `&[String]` is five compile errors at the call sites.

    **`schema.rs` — the number that was right, and the cause behind it.** `examples.len() == 15` was correct; `test_operations()`'s 14 was a hand-copied SUBSET of the 25-op production roster, so the two assertions never actually contradicted each other and no test was failing. The defect the finding names is the second roster. `test_operations()` now returns `code_context_operations()` — the production list — and the two counts derive from `ops.len()` instead of restating a literal. A new test, `test_every_example_names_an_operation_on_the_roster`, pins the invariant that was silently broken: an example may only name an op the tool really dispatches. RED three ways: `test_operations()` truncated back to 14 ops fails the new roster test (the exact defect); the schema config stripped of `.with_examples(...)` fails the examples test; the roster accessor truncated to 24 fails the tool's own op-count test.

    **`doctor.rs:682` — the case-sensitive stem.** Extracted as `is_sah_binary(&Path)` and compared with `eq_ignore_ascii_case`, with the reason written down: Windows resolves `SAH.EXE`, `Sah.exe` and `sah.exe` to one file, so a case-sensitive test declines the very binary it is looking for and falls through to whatever older copy sits first on `PATH`. `the_binary_stem_test_reads_the_name_and_not_its_case` drives eight spellings through it. The first draft of that table used `C:\Program Files\...` and failed on macOS for the wrong reason — `Path::file_stem` treats a backslash as an ordinary character, so the stem was the whole string; the rows are written with `/` so the same stems are exercised on every platform and CASE is what is under test. RED: the comparison put back to `==` fails the test.

    **`doctor.rs:697` and `:521` — the shared shell.** `prepare_shell_command` lives in `swissarmyhammer-tools`, which DEPENDS ON `swissarmyhammer-validators`, so validators cannot import it; the shared core went down into `swissarmyhammer-common` instead, as `command::shell_command(Shell, &str) -> Command` plus `command::command_failure_detail(&Output) -> String`. Four callers now go through them: `doctor::run_shell` (adds `"$@"` and `SAH_BIN`, which is all that was ever its own), `shell/process.rs::prepare_shell_command` (adds the working directory and the environment, and converts to `tokio::process::Command`), and the two hand-written copies of "stderr when present, status otherwise" in `code_context/doctor.rs` and `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs`. `swissarmyhammer-validators` gained `swissarmyhammer-common` as a real dependency; it was already a dev-dependency, and the dev entry was folded into the real one rather than left duplicated. RED five ways on the new module: bash resolved to `sh`; the platform shell forced to bash; the failure summary always the status; always the stderr; and the bash flag wrong.

    **`doctor.rs:423` — `run_fixture` and `run_tool_script`.** There is no symbol named `fixture_replay` anywhere in the tree, so the finding named the concept; the concept was real. `run_fixture` (doctor) and `run_tool_script` (review engine) were two implementations of the same five steps. Both now call `tool_rules::run_script_findings`, with `tool_rules::script_args` deciding the positional arguments from the scope. The failure comes back as a `ScriptFailure` — `Start`, `Exit`, `Contract` — so the runner reports WHAT went wrong and each caller still writes its own sentence: the doctor names the fixture it was proving, the engine names the rule it was running.

    **A gap the RED sweep found and closed.** Breaking `run_script_findings` so a nonzero exit reads as a clean run left the whole doctor suite GREEN — no test anywhere exercised a tool-rule script that fails. That is load-bearing behaviour the extraction now owns, so `BROKEN_TOOL_RULE` (a script that writes to stderr and exits 4) and `test_a_nonzero_exit_is_reported_as_a_broken_tool_with_its_own_words` were added. RED two ways: the exit check removed, and the detail reduced to the status instead of the script's own words.

    **Gates.** `cargo nextest run --workspace` — 13906 passed, 0 failed, 0 skipped (13898 before this pass; the eight new tests are five in `common::command`, the stem test, the example-roster test, and the broken-tool test). `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.

    **RED verified 17 ways** — 4 dependency restorations, 1 signature revert (compile), 3 schema mutations, 1 dispatch misroute, 1 stem-comparison revert, 1 `SAH_BIN` export removal, 5 mutations of the shared command module, and 2 of the shared script runner. Every one went RED and every one was restored to GREEN.
  timestamp: 2026-08-08T23:25:57.564590+00:00
- actor: claude-code
  id: 01kzhv5an1gv5qea5j8g5f52dw
  text: |-
    ### implement — changed
    - evidence: 21 files — 8 new (`crates/swissarmyhammer-common/src/command.rs`, `code_context/{ops,support,execute,indexing,status,lsp_ops}.rs`, `code_context/tests/` as 6 files), 13 modified. All 12 review findings now `- [x]`. `cargo nextest run --workspace` 13906 passed / 0 failed / 0 skipped; `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean. RED verified 17 ways.
    - next: `/review`
  timestamp: 2026-08-08T23:26:04.961522+00:00
- actor: claude-code
  id: 01kzj0anrhf7x9k98rya27eyy8
  text: |
    ### review — findings

    - evidence: 38 findings recorded, from `review sha d861843cf..385feb8c4` (engine: 61 findings, 62 confirmed, 73 refuted, 54 attempted, 0 failed, **0 skipped files**). Locations: `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs:327`; `crates/swissarmyhammer-code-context/src/ops/find_commented_code.rs:58`; `crates/swissarmyhammer-common/src/lib.rs:49`, `:128`; `code_context/doctor.rs:14`, `:15`, `:16`, `:17`, `:25`, `:26`, `:27`, `:35`; `code_context/execute.rs:89`, `:258`, `:442`; `code_context/indexing.rs:465`; `code_context/lsp_ops.rs:32`; `code_context/mod.rs:65`, `:182` (x2), `:229`, `:240`, `:313`, `:318`; `code_context/ops.rs:32`, `:66`, `:125`, `:191`, `:220`, `:252`, `:370`, `:807`; `code_context/support.rs:82`; `code_context/watcher.rs:158`; `shell/process.rs:22`, `:177`; `crates/swissarmyhammer-validators/Cargo.toml:17`, `:18`.
    - next: `/implement` the 38 checklist items, then re-review. Task stays in `review`.

    **The 12 prior findings are genuinely fixed, each verified in the tree — not merely checked off.**

    - Cargo.toml (4): `grep` over `crates/swissarmyhammer-code-context/Cargo.toml` for `project-detection`, `model-embedding`, `once_cell`, `include_dir` returns nothing. All four absent.
    - `find_commented_code.rs:48`: signature now reads `pub fn find_commented_code(working_dir: &Path, files: &[&str])`.
    - `mod.rs:1`: the file is 337 lines / 14297 bytes, down from 4890 / 187624. Proof it worked is in this report's own counts — **`skipped_files` is empty and `skipped` is 0**, where the last pass skipped this file and nine validators never read it.
    - `schema.rs:100`/`:169`: `test_operations()` is now `code_context_operations().to_vec()` — the production roster — and the two assertions derive from `ops.len()` instead of a literal. The second hand-copied roster is gone.
    - `doctor.rs:423`: `run_fixture` and the engine's `plan_tool_rules` both call `tool_rules::run_script_findings` with `script_args`, and each maps `ScriptFailure::{Start,Exit,Contract}` into its own sentence.
    - `doctor.rs:521`: exactly one `fn command_failure_detail` exists in the tree, in `swissarmyhammer-common/src/command.rs`. Four callers import it; the two hand-written copies are gone.
    - `doctor.rs:682`: `is_sah_binary` compares with `eq_ignore_ascii_case`, with the Windows reason written above it.
    - `doctor.rs:697`: `shell_command(Shell, &str)` in common; `doctor::run_shell` and `shell/process.rs::prepare_shell_command` both build on it.

    **23 findings dropped under the pre-existing-test exception**, each confirmed present at `d861843cf` before being dropped:

    - `code_context/tests/indexer.rs:21`, `:56`, `:108`, `:204`, `:247`, `:251`, `:299`; `tests/support.rs:147`; `tests/tool.rs:35` (x2), `:98` — 11. The `tests/` directory is a **pure move**: 50 `#[test]`/`#[tokio::test]` functions in the old `mod.rs`, 50 in the new `tests/` directory (13+9+2+26). Every flagged literal — `620_000_000`, `MockEmbedder::new(8)`, `let dim = 8`, `VecReporter`, `read_embedded_flag`, `ops.len(), 25` — was located in `mod.rs` at `d861843cf`.
    - `watcher.rs:411`, `:760` — 2. Both sit past `mod tests` at line 372, and this commit changed only three call paths in that file.
    - `shell/process.rs:724`, `:754`, `:801`, `:844`, `:927`, `:951`, `:957`, `:1137`, `:1395` — 9. All sit past `#[cfg(test)]` at line 669; this commit touched only `prepare_shell_command`.
    - `review/tool_install.rs:499` — 1. Past `#[cfg(test)]` at line 456; this commit changed only the import block.

    **Three notes for the next iteration.**

    The split is the whole story of this report. Nine validators read `code_context/*` for the first time and returned 27 findings across `mod.rs`, `ops.rs`, `execute.rs`, `lsp_ops.rs`, `indexing.rs` and `support.rs` — moved code, not new code, that no review had ever been able to see. The implementer predicted one of these exactly: the two verbatim `"Valid operations: ..."` literals kept unchanged on purpose are now raised as `mod.rs:313` and `:318`. That is the split working as designed.

    `find_commented_code.rs:58` is the one finding of substance specific to this change and the only security finding: the op joins a caller-supplied path to `working_dir` with no containment check, so `--files ../../../etc/passwd` reads outside the workspace. This is the file whose signature this iteration changed, and the path is reachable from the MCP op and the `sah tool` CLI.

    `swissarmyhammer-validators/Cargo.toml:17`, `:18` are the same cause as prior findings 1–4, in the next package: the dev-dependency fold put this manifest in the diff, so cargo-machete read it and found `sha2` and `chrono` unused.

    The new shared module `swissarmyhammer-common/src/command.rs` drew **zero** findings, as did the six new sibling modules' own structure. The deliberate `stdin(Stdio::null())` is documented at the function and drew no finding; `shell/process.rs` set stdin null before the change, so the user-facing shell tool keeps its behavior.

    One clarification the engine's text does not make plain: `crates/swissarmyhammer-common/src/lib.rs:49` is `pub mod test_utils;`, not the new `pub mod command;` on line 23. Neither declaration carries a doc comment.
  timestamp: 2026-08-09T00:56:23.057082+00:00
- actor: claude-code
  id: 01kzj0d00waqcgdk9xf67hg4vp
  text: |-
    ### finish iteration 2 — findings
    - implement: changed — 21 files; all 12 prior findings worked, code_context/mod.rs split 4890 -> 337 lines, new shared swissarmyhammer-common::command module, unsafe transmute removed. RED verified 17 ways
    - test: green — cargo nextest run --workspace, 13906 passed, 0 failed, 0 skipped; fmt clean; clippy -D warnings clean; cargo check --all-features clean. The +8 delta from 13898 was reconciled test-by-test and all 50 tests from the old mod.rs confirmed present
    - commit: 385feb8c4 — 31 files, 5336 insertions, 4758 deletions
    - review: findings — 38 open, on `review sha d861843cf..385feb8c4`. Engine: 61 returned, 62 confirmed, 73 refuted, 54 attempted, 0 failed, **0 skipped**; 23 dropped under the pre-existing-test exception

    The 12 prior findings were verified in the tree rather than trusted from their checkboxes: the four deps are gone from the manifest, `find_commented_code` reads `&[&str]`, `test_operations()` derives from the production roster, exactly one `fn command_failure_detail` exists workspace-wide, `is_sah_binary` uses `eq_ignore_ascii_case`, and both runners route through `run_script_findings`.

    **The split is why this number jumped.** `skipped` is 0 and `skipped_files` is empty, where the previous pass had nine validators unable to read `code_context/mod.rs` at all. **27 of the 38 findings are in `code_context/*` — relocated code, not new code, that no review had ever seen.** The implementer predicted one of them exactly: the two verbatim `"Valid operations: ..."` literals left unchanged on purpose are now `mod.rs:313` and `:318`.

    **The one finding of substance specific to this change, and the only security finding:** `crates/swissarmyhammer-code-context/src/ops/find_commented_code.rs:58` joins a caller-supplied path to `working_dir` with no containment check, so `--files ../../../etc/passwd` reads outside the workspace. That path is reachable from both the MCP op and `sah tool`. This is the file whose signature iteration 2 changed.

    `crates/swissarmyhammer-validators/Cargo.toml:17, :18` are the same cause as prior findings 1-4, one package over: the dev-dependency fold put that manifest in the diff, and cargo-machete then found `sha2` and `chrono` unused.

    Two pieces drew zero findings: the new shared `command.rs` module and the `unsafe { std::mem::transmute }` removal.

    Findings by file:
    - apps/swissarmyhammer-cli/src/commands/doctor/checks.rs:327
    - crates/swissarmyhammer-code-context/src/ops/find_commented_code.rs:58
    - crates/swissarmyhammer-common/src/lib.rs:49, :128
    - code_context/doctor.rs:14, :15, :16, :17, :25, :26, :27, :35
    - code_context/execute.rs:89, :258, :442
    - code_context/indexing.rs:465
    - code_context/lsp_ops.rs:32
    - code_context/mod.rs:65, :182 (x2), :229, :240, :313, :318
    - code_context/ops.rs:32, :66, :125, :191, :220, :252, :370, :807
    - code_context/support.rs:82
    - code_context/watcher.rs:158
    - crates/swissarmyhammer-tools/src/mcp/tools/shell/process.rs:22, :177
    - crates/swissarmyhammer-validators/Cargo.toml:17, :18

    Guardrail: NOT hit. No finding repeats from iteration 1 — these are 38 different findings, and the 12 prior ones are closed.
  timestamp: 2026-08-09T00:57:39.100021+00:00
- actor: claude-code
  id: 01kzj0ssct99fh0mzfnxek2q24
  text: |
    ### Iteration 3 — the security finding, and the sibling op that shared its cause

    `find_commented_code.rs:58` was real and it was reachable. `working_dir.join(file)` reads whatever the caller names: a relative `../outside.rs` climbs out of the workspace, and an absolute `/etc/passwd` replaces the base entirely, because `Path::join` discards the base when the argument is absolute. Both paths arrive through the MCP op and through `sah tool code_context commented_code find --files`.

    **The sweep found ONE sibling with the same cause, and it is not the one the grep first suggests.** `crates/swissarmyhammer-code-context/src/ops/query_ast.rs` joins caller-supplied `file_paths` to `workspace_root` with no containment check either. `lsp_helpers.rs::read_source_range` also reads a path off `std::fs`, but that path comes back from the language server's own response rather than from the caller, so it is not this cause and was left alone. Every other op in `ops/` reads through the index, not the filesystem.

    **The fix is one place, not two patches.** New module `crates/swissarmyhammer-code-context/src/ops/workspace_path.rs`, holding `resolve_within(working_dir, file) -> Option<PathBuf>`. It canonicalizes both sides and requires the resolved path to start with the resolved root. Canonicalizing rather than string-matching on `..` is what makes it cover the third shape the finding's own text does not name: a symbolic link inside the workspace that points out of it. An absolute path that lands INSIDE the workspace still resolves, so the boundary is where the path lands and not how the caller spelled it. A refusal is logged at `warn` through `tracing`.

    **RED verified 4 ways before the fix, on the production path.** `a_relative_path_that_climbs_out_of_the_working_directory_is_refused` and `an_absolute_path_outside_the_working_directory_is_refused` in `find_commented_code`, and the matching pair in `query_ast`, all failed against the old code — `query_ast` reported `files_scanned: 1` for a file outside its workspace, which is the read happening. All four are green now, beside 7 new unit tests on `resolve_within` itself (inside relative, inside absolute, relative climb, absolute outside, a climb that returns inside, a symbolic-link escape, and a missing file).

    One existing test changed rather than being deleted: `an_absolute_path_is_read_where_it_lies` pointed a nonexistent `working_dir` at a file elsewhere on disk, which is the defect written as a test. It is now `an_absolute_path_inside_the_working_directory_is_read` and pins the behaviour that survives.
  timestamp: 2026-08-09T01:04:38.298848+00:00
- actor: claude-code
  id: 01kzj215mh13qfjtycr6b52cmv
  text: |
    ### Iteration 3 — the other 37, and what each cause turned out to be

    Each finding was worked as a CAUSE over the whole file, never as the cited line. Every claim below was re-verified in the tree after the work, not taken from a report.

    **`code_context/mod.rs` (7 findings).** `CodeContextTool` derives `Debug`. Three constants now own the three different spellings the findings named — `CODE_CONTEXT_DIR` (`.code-context`), `CODE_CONTEXT_DIR_WITH_SLASH` (`.code-context/`) and `CODE_CONTEXT_INIT_NAME` (`code-context`) — and the sweep routed all of them: both `root.join()` sites, the `.gitignore` comparison and the line that appends the entry, and all seven `InitResult` name arguments. A grep for the literal family now matches the three `const` definitions and nothing else. The two verbatim 600-character operation lists are one `VALID_OPERATIONS_LIST` constant interpolated at both error sites — the prediction the last iteration wrote down, now closed.

    **`code_context/ops.rs` (8 findings).** One cause: the operation roster spelled its nouns and verbs inline. 31 constants (10 verbs, 21 nouns) now cover all 50 `verb()`/`noun()` bodies, single-use ones included, so no operation can be spelled two ways. The 25 hand-written `Lazy` statics and the 25-entry vector became one `macro_rules! code_context_roster` invocation that emits the singleton and the roster entry together, so an operation is named exactly once. Roster ORDER was checked positionally against `git show HEAD:...ops.rs` — 25/25, no mismatch — because the order is a public contract.

    **`code_context/execute.rs` + `lsp_ops.rs` + `support.rs` (5 findings).** One cause seen three times: JSON argument extraction written longhand at every call site. Twelve `extract_*` helpers now live in `support.rs`, including `extract_u32_param` and `extract_file_position` with the exact signatures the findings prescribe. **`args.get(` now appears zero times in either file** — that is the measurement that says the cause is gone rather than the three cited sites. Error text is byte-identical, so no caller-visible message changed.

    **`code_context/doctor.rs` (8 findings).** Every public item in the file is documented, not only the seven cited lines, and `run_doctor` now calls `detect_project_types(root)` instead of repeating its `format!`/`to_lowercase` transformation. Proof: `cargo clippy -p swissarmyhammer-tools --lib -- -W missing_docs` reports **0** diagnostics anywhere under `code_context/`.

    **Unnamed configuration literals (5 findings, 4 files).** `DEFAULT_MIN_SIMILARITY`, `INDEXING_PROGRESS_LOG_INTERVAL`, `SHUTDOWN_CHECK_INTERVAL_MILLIS`, `PROCESS_REAP_TIMEOUT_MILLIS`, `DETECTION_DEPTH` — plus every sibling literal the same sweep found: five more defaults in `execute.rs`, and `PROCESS_REAP_POLL_MILLIS` / `SIGNAL_TERMINATED_EXIT_CODE` in `shell/process.rs`.

    **`swissarmyhammer-common/src/lib.rs` (2 findings).** The reviewer's correction was right and was checked before acting: line 49 is `pub mod test_utils;`, not `pub mod command;`. Neither carried a doc comment, and neither did the other 31 module declarations — **all 33 are documented now**. `format_pretty` is extracted and both `Display::fmt` and `Debug::fmt` call it.

    **`shell/process.rs` (2 findings).** `tokio::process::Child` DOES implement `Debug`, so `AsyncProcessGuard` derives it rather than taking the hand-written fallback the finding offered for the case where it does not — the derive reports strictly more state.

    **`swissarmyhammer-validators/Cargo.toml` (2 findings).** Proved before deleting: a case-insensitive grep for `sha2|Sha256|chrono|NaiveDate|DateTime|Utc::|Local::` over the whole package returns 5 hits and every one is the substring "chrono" inside the word "sync**hrono**us" — the same false positive the rule body documents. Both dependencies deleted, together with the now-false comment above them, following the `swissarmyhammer-code-context` precedent from iteration 2; no `[package.metadata.cargo-machete]` table added. `cargo machete crates/swissarmyhammer-validators` now says "didn't find any unused dependencies".

    **One deviation, stated plainly.** `checks.rs:327` asks for `const DETECTION_DEPTH: u32 = 3;`. `swissarmyhammer_project_detection::detect_projects` takes `max_depth: Option<usize>`, so `u32` does not compile. The constant is `usize`. The finding's requirement — the literal is named at module level and used as `Some(DETECTION_DEPTH)` — is met exactly; only the integer type differs, because the rule as literally written cannot type-check.

    **One extra copy closed that no finding cited.** The `support.rs:82` sweep found the working-dir-then-git-root resolution a THIRD time in `execute.rs::execute_query_ast` and a FOURTH time in `code_context/detect.rs::resolve_workspace_path`. Both now call `resolve_workspace_root`, so the cause is gone from the directory rather than from the two lines the finding named.

    **Gates.** `cargo nextest run --workspace` — **13917 passed, 0 failed, 0 skipped** (13906 before this pass; the +11 are the security tests). `cargo fmt --all --check` clean. `cargo clippy --workspace --all-targets -- -D warnings` clean.

    **RED verified 4 ways**, all on the security defect and all on the production path: the relative climb and the absolute escape, in `find_commented_code` and again in `query_ast`. The other 37 findings are structural — a missing doc comment, a repeated literal, an unused manifest entry — and the compiler, `clippy -W missing_docs`, and `cargo machete` are their gates rather than a failing test.
  timestamp: 2026-08-09T01:26:08.785568+00:00
- actor: claude-code
  id: 01kzj21en5p7zbh6zntpwyqjck
  text: |-
    ### implement — changed
    - evidence: 17 source files + 1 new module. New: `crates/swissarmyhammer-code-context/src/ops/workspace_path.rs`. Modified: `code-context/src/ops/{find_commented_code,query_ast,mod}.rs`; `code_context/{mod,ops,execute,lsp_ops,support,doctor,indexing,watcher,detect}.rs`; `shell/process.rs`; `swissarmyhammer-common/src/lib.rs`; `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs`; `swissarmyhammer-validators/Cargo.toml`; `Cargo.lock`. All 38 review findings now `- [x]`. `cargo nextest run --workspace` 13917 passed / 0 failed / 0 skipped; `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets -- -D warnings` clean. RED verified 4 ways on the path-traversal defect, in two ops.
    - next: `/review`
  timestamp: 2026-08-09T01:26:18.021480+00:00
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

## Review Findings (2026-08-08 17:13)

> ⚠️ 1 file(s) not reviewed — the rendered prompt would exceed the agent's prompt cap:
> - `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs` — 520167 rendered bytes, over the 262144-byte per-file cap; not reviewed by: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity (split the file)

- [x] `crates/swissarmyhammer-code-context/Cargo.toml:15` — unused dependency `swissarmyhammer-project-detection`: no source file of this package names it; delete it, or list it under `[package.metadata.cargo-machete] ignored` with a comment saying why.
- [x] `crates/swissarmyhammer-code-context/Cargo.toml:28` — unused dependency `model-embedding`: no source file of this package names it; delete it, or list it under `[package.metadata.cargo-machete] ignored` with a comment saying why.
- [x] `crates/swissarmyhammer-code-context/Cargo.toml:33` — unused dependency `once_cell`: no source file of this package names it; delete it, or list it under `[package.metadata.cargo-machete] ignored` with a comment saying why.
- [x] `crates/swissarmyhammer-code-context/Cargo.toml:34` — unused dependency `include_dir`: no source file of this package names it; delete it, or list it under `[package.metadata.cargo-machete] ignored` with a comment saying why.
- [x] `crates/swissarmyhammer-code-context/src/ops/find_commented_code.rs:48` — Function accepts `&[String]` (concrete type) instead of `&[&str]` (borrowed reference), forcing callers to convert string literals with `.to_string()` rather than passing them directly. This violates the guideline to accept borrowed references instead of owned concrete types for better API flexibility. Change signature from `pub fn find_commented_code(working_dir: &Path, files: &[String])` to `pub fn find_commented_code(working_dir: &Path, files: &[&str])`. The internal `findings_in_file` function at line 57 already accepts `&str`, so this change is a simple pass-through of borrowed references.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:1` — This file exceeds the review prompt cap — 520167 rendered bytes against the 262144-byte per-file cap — so these validators could not review it: code-hygiene, code-security, completeness, duplication, magic-numbers, naming, reuse, rust, test-integrity. Split the file into smaller modules that fit the review prompt cap.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/schema.rs:100` — The imports list at lines 100–103 does not include FindCommentedCode, yet the examples added at lines 62–64 reference the operation "find commented_code", and line 282 asserts examples.len() == 15. The test_operations() function at lines 105–122 provides only 14 operations (unchanged in this commit), so the schema validation will either fail or be inconsistent. Either (1) add FindCommentedCode to the imports and to test_operations(), or (2) remove the commented_code example from lines 62–64 if the operation has not yet been defined in code_context.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/schema.rs:169` — New 'find commented_code' example added (line 62-64) and example count assertion updated to 15 (line 282), but test_operations() still lists only 14 operations. The schema generation test expects 14 ops, creating a mismatch with 15 documented examples. If FindCommentedCode is a new operation, it must be added to test_operations(). Add FindCommentedCode (or the corresponding operation struct) to the test_operations() array (lines 105-122), making it 15 operations. Then update assertions on lines 169 and 192 from 14 to 15 to match.
- [x] `crates/swissarmyhammer-validators/src/doctor.rs:423` — Existing `run_fixture()` function duplicates fixture execution logic that already exists elsewhere as `fixture_replay`; similar test fixture running implementations should be consolidated. Extract fixture execution logic to a shared utility so fixture testing in both validators and conformance tests reuse the same implementation.
- [x] `crates/swissarmyhammer-validators/src/doctor.rs:521` — Existing `command_failure_detail()` function duplicates command error extraction logic; similar implementations already exist elsewhere. Extract command failure formatting into a shared utility in swissarmyhammer-common so both production and test code reuse the same error reporting logic.
- [x] `crates/swissarmyhammer-validators/src/doctor.rs:682` — The new comparison `exe.file_stem() == Some(OsStr::new(SAH_BINARY_NAME))` is case-sensitive, but executable file names on Windows are case-insensitive — if the binary is installed as `SAH.exe` or `Sah.exe`, the comparison fails even though the filesystem treats them identically. The added tests do not verify that non-canonical case variations are handled or rejected consistently across platforms. Add one regression test that mocks or verifies the behavior when `current_exe()` returns an uppercase-stemmed path (e.g., by setting `SAH_BIN` to `SAH` in the environment and confirming the fallback is used), or document that the executable name is guaranteed lowercase by the build process with a comment and a test asserting that contract.
- [x] `crates/swissarmyhammer-validators/src/doctor.rs:697` — Existing `run_shell()` function duplicates shell command execution logic that exists elsewhere as `prepare_shell_command`; should reuse or extend existing implementation instead of maintaining a parallel copy. Investigate whether `run_shell()` can be replaced with `prepare_shell_command` or refactored to reuse it. Keeping one canonical shell runner implementation ensures consistent behavior and maintenance burden across production and test code paths.

## Review Findings (2026-08-08 18:37)

- [x] `apps/swissarmyhammer-cli/src/commands/doctor/checks.rs:327` — Hardcoded depth limit of 3 for project detection should be a named constant to enable easy reconfiguration and improve maintainability. Extract as `const DETECTION_DEPTH: u32 = 3;` at the module level and use `Some(DETECTION_DEPTH)` in the call.
- [x] `crates/swissarmyhammer-code-context/src/ops/find_commented_code.rs:58` — Path traversal vulnerability: file path parameter is joined directly to working_dir without validation, allowing callers to read arbitrary files by passing paths like '../../../etc/passwd' or '/etc/passwd'. Validate each file path to ensure it does not contain '..' components and is not absolute. Use `std::path::Path::canonicalize()` on the joined path and verify it is within `working_dir` before reading, or use `dunce::canonicalize()` for cross-platform compatibility. Example: `let canonical = working_dir.join(file).canonicalize()?; ensure!(canonical.starts_with(working_dir.canonicalize()?), "path traversal attempt");`.
- [x] `crates/swissarmyhammer-common/src/lib.rs:49` — missing documentation for a module.
- [x] `crates/swissarmyhammer-common/src/lib.rs:128` — Display and Debug implementations for Pretty<T> have identical bodies (lines 128-135 and 137-144). The same formatting logic repeats in both trait impls and could drift out of sync. Extract the formatting logic into a shared helper function `fn format_pretty<T: Serialize + Debug>(obj: &T, f: &mut Formatter) -> fmt::Result` and call it from both Display::fmt and Debug::fmt impls.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/doctor.rs:14` — missing documentation for a struct.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/doctor.rs:15` — missing documentation for a struct field.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/doctor.rs:16` — missing documentation for a struct field.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/doctor.rs:17` — missing documentation for a struct field.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/doctor.rs:25` — missing documentation for a struct.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/doctor.rs:26` — missing documentation for a struct field.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/doctor.rs:27` — missing documentation for a struct field.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/doctor.rs:35` — Project type transformation logic repeats at lines 35-37 (detect_project_types) and lines 79-82 (run_doctor). Both apply identical transformation: map(|pt| format!("{:?}", pt).to_lowercase()) over the result of detect_project_type_enums. In run_doctor, replace lines 79-82 with: `let project_types: Vec<String> = detect_project_types(root);` This eliminates code duplication and keeps the transformation in one place.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/execute.rs:89` — File_path parameter extraction repeats identically at lines 89-92 (execute_list_symbols) and 249-252 (execute_find_duplicates). Same four-line block structure and error message. Use the same helper function proposed above or create a specialized one for string parameters. This pattern also duplicates the file_path extractions already identified in lsp_ops.rs.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/execute.rs:258` — Hardcoded default value of 0.85 for min_similarity threshold in duplicate detection should be a named constant to make the similarity threshold configurable. Extract as `const DEFAULT_MIN_SIMILARITY: f32 = 0.85;` and use it in the `unwrap_or()` call.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/execute.rs:442` — Numeric parameter extraction pattern repeats near-identically at lines 442-446 (max_depth in execute_get_callgraph) and 480-484 (max_hops in execute_get_blastradius). Both use: args.get(param).and_then(|v| v.as_u64()).map(|n| n as u32).unwrap_or(default). Differ only by parameter name and default value. Extract into a helper function: `fn extract_u32_param(args: &Map<String, Value>, param: &str, default: u32) -> u32` and call from both functions instead of repeating the identical transformation.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/indexing.rs:465` — Hardcoded interval of 100 for progress logging checkpoints should be a named constant to make the logging frequency configurable. Extract as `const INDEXING_PROGRESS_LOG_INTERVAL: u64 = 100;` and use it in the `is_multiple_of()` call.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/lsp_ops.rs:32` — Parameter extraction for file_path, line, and character repeats identically across seven functions (execute_get_rename_edits, execute_get_inbound_calls, execute_get_definition, execute_get_type_definition, execute_get_hover, execute_get_references, execute_get_implementations). The three-line block for file_path extraction at lines 32-35 repeats at lines 118-121, 197-200, 243-246, 290-293, 336-339, 388-391 with zero variation. Extract parameter extraction into a helper function: `fn extract_file_position(args: &Map<String, Value>) -> Result<(String, u32, u32), McpError>` and call it from all seven functions instead of repeating the three-parameter extraction block.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:65` — Public struct CodeContextTool is missing Debug implementation, which is required for all public types to enable logging, debugging contexts, and allow downstream crates to add their own Debug-dependent traits without orphan rule issues. Change line 65 from `#[derive(Clone, Default)]` to `#[derive(Clone, Debug, Default)]`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:182` — The string literal ".code-context" appears in a condition check and is repeated elsewhere; should use a named constant. Replace with a named constant to ensure consistent maintenance.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:182` — The string literal ".code-context/" appears in this line's string comparison and is repeated elsewhere; part of the ".code-context/" repeated literal family. Define a const: `const CODE_CONTEXT_DIR_WITH_SLASH: &str = ".code-context/";` and use in all comparisons and output.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:229` — The string literal "code-context" appears six times as a status ID and should be a named constant. Replace with a module-level named constant.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:240` — The string literal "code-context" appears six times and should be extracted to a named constant. Replace with a module-level named constant to ensure consistent maintenance.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:313` — Operation list string is duplicated verbatim on line 318. If the list of valid operations changes, it must be updated in both places or they drift out of sync. Extract the operations list to a module-level constant, e.g., `const VALID_OPERATIONS_LIST: &str = "'get symbol', 'search symbol', ...";` and use it in both error messages to maintain a single source of truth.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/mod.rs:318` — Operation list string is duplicated verbatim on line 313. If the list of valid operations changes, it must be updated in both places or they drift out of sync. Extract the operations list to a module-level constant, e.g., `const VALID_OPERATIONS_LIST: &str = "'get symbol', 'search symbol', ...";` and use it in both error messages to maintain a single source of truth.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/ops.rs:32` — The string literal "symbol" appears multiple times as a noun in Operation implementations and should be a named constant. Define a const: `const NOUN_SYMBOL: &str = "symbol";` and use in both GetSymbol and SearchSymbol implementations.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/ops.rs:66` — The string literal "symbol" is repeated; should use a named constant for single-point maintenance. Replace with a module-level named constant.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/ops.rs:125` — The string literal "code" appears twice as a noun in Operation implementations and should be a named constant. Define a const: `const NOUN_CODE: &str = "code";` and use in both GrepCode and SearchCode implementations.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/ops.rs:191` — The string literal "get" appears 13 times across Operation implementations and should use a named constant. Replace with a module-level named constant.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/ops.rs:220` — The string literal "search" is repeated three times and should use a named constant. Replace with a module-level named constant.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/ops.rs:252` — The string literal "get" is repeated 13 times and should use a named constant. Replace with a module-level named constant.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/ops.rs:370` — The string literal "code" is repeated twice and should use a named constant. Replace with a module-level named constant.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/ops.rs:807` — All 25 static Lazy operation instances (lines 807–832) follow an identical pattern: `static NAME_OP: Lazy<Type> = Lazy::new(Type::default);` differing only in the operation name and type. This duplication compounds when the new vector on line 835 must list all 25 operations separately. Use a macro to generate both the static Lazy instances (lines 807–832) and the CODE_CONTEXT_OPERATIONS vector entries (lines 835–861). For example, a `declare_operation!` macro could generate the Lazy instance and the vector entry from a single invocation per operation, eliminating both duplication sources and the risk of drift when an operation is added or removed.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/support.rs:82` — Lines 82–88 in `open_workspace` are verbatim duplicated at lines 204–208 in `maybe_append_lsp_notice`. Both blocks resolve the working directory from the context and find the git repository root using identical code. Extract a helper function `fn resolve_workspace_root(context: &ToolContext) -> PathBuf` to eliminate the duplication. Extract a helper function `fn resolve_workspace_root(context: &ToolContext) -> PathBuf { let working_dir = context.working_dir.clone().unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| ".".into())); find_git_repository_root_from(&working_dir).unwrap_or(working_dir) }`, then call it from both `open_workspace` and `maybe_append_lsp_notice`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/watcher.rs:158` — Hardcoded timeout value 500 ms for shutdown check interval should be a named constant to clarify intent and enable easy tuning. Extract to a module-level constant: `const SHUTDOWN_CHECK_INTERVAL_MILLIS: u64 = 500;` and use `Duration::from_millis(SHUTDOWN_CHECK_INTERVAL_MILLIS)`.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/process.rs:22` — Public struct AsyncProcessGuard does not implement Debug trait. All public types with non-empty representation must implement Debug for runtime introspection and debugging support. Implement Debug for AsyncProcessGuard. If Child does not implement Debug, write a custom impl that debugs the command field and process state: impl fmt::Debug for AsyncProcessGuard { fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result { f.debug_struct("AsyncProcessGuard").field("command", &self.command).finish() } }.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/shell/process.rs:177` — Hardcoded timeout 100 ms in Drop implementation should be a named constant for clarity and maintenance. Define as constant: `const PROCESS_REAP_TIMEOUT_MILLIS: u64 = 100;`.
- [x] `crates/swissarmyhammer-validators/Cargo.toml:17` — unused dependency `sha2`: no source file of this package names it; delete it, or list it under `[package.metadata.cargo-machete] ignored` with a comment saying why.
- [x] `crates/swissarmyhammer-validators/Cargo.toml:18` — unused dependency `chrono`: no source file of this package names it; delete it, or list it under `[package.metadata.cargo-machete] ignored` with a comment saying why.
