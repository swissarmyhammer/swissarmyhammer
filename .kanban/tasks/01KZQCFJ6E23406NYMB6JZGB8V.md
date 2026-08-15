---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01m0116d4a6bn87kajv4d8vsxc
  text: |-
    Research, measured on revive 1.15.0, staticcheck 2025.1.1, golangci-lint 2.12.2, go 1.26.5.

    TOOL SURVEY (checklist item 3). `revive`'s `exported` rule holds the repetitive-name check alone.
    - revive: 12 rules write `FailureCategoryNaming` — confusing-naming, confusing-results, epoch-naming, error-naming, exported, import-shadowing, package-directory-mismatch, package-naming, receiver-naming, unexported-naming, use-any, var-naming. Each of the other 11 was run over one probe file holding a documented stuttering type, an undocumented stuttering type, an underscore name, a name equal to the package name, a name whose next rune is lower case, a stuttering const, a stuttering var and a stuttering method. Ten are silent; `var-naming` reports the UNDERSCORE only ("don't use underscores in Go names"), which is a different defect.
    - staticcheck `-checks all` over the same probe reports ST1000 (package comment), ST1003 (the same underscore) and U1000 (unused). No stutter.
    - golangci-lint `default: all` = 115 linters enabled, run over the same probe: only `revive` reports the stutter, and `unused` reports the dead type. The naming-adjacent linters in the roster — predeclared, errname, importas, inamedparam, varnamelen, nonamedreturns, tagliatelle, promlinter, goprintffuncname, gocritic, asciicheck — report nothing here.

    WHERE THE CHECK LIVES. `rule/exported.go` calls `checkRepetitiveNames` from `Visit` for a `*ast.FuncDecl` with no receiver and for a `*ast.TypeSpec`, and nowhere else. It writes `lint.FailureCategoryNaming`; every other failure of that rule writes `FailureCategoryComments`. `Apply` returns early on `!file.IsImportable()`, which is `_test.go` OR `package main`, and `lint/linter.go` skips a generated file before that. So the three carve-outs cover the naming half exactly as they cover the comments half.

    MEASURED over one probe file (a documented stuttering type, a plain undocumented type, an undocumented stuttering type, `Staged` equal to the package name, `Stagedly`, `Staged_Thing`, an unexported type, a stuttering const, a stuttering var, a stuttering func, a stuttering method, a plain func):
    - `[rule.exported]` with no argument: 10 `comments` + 4 `naming`.
    - `arguments = ["disableStutteringCheck"]` (the shipped missing-docs-go config): 10 `comments`, 0 `naming`.
    - The 4 naming findings are the documented stuttering type, the undocumented stuttering type, `Staged_Thing`, and the stuttering func. `Staged` (equal length), `Stagedly` (lower-case next rune), the const, the var, the method and the unexported type each report no name.
    - `sayRepetitiveInsteadOfStutters` rewrites "and that stutters" to "and that is repetitive" and leaves `Category` at `naming` (checklist item 2). A filter on the word breaks; a filter on `Category` does not.

    REVIVE HAS NEITHER OF golangci-lint's HAZARDS (the question cards ^1h52223 / ^mms9g8d raise).
    - No lock: 8 runs of the shipped-shape script started together over one workspace each reported all 4 findings, over two rounds.
    - No cache: 400 packages in two directories holding the same bytes — 0.04 s / 0.04 s / 0.05 s / 0.05 s over first-cold, first-warm, second-cold, second-warm, so the second directory paid no cold cost and read no cached answer. Each directory reported its own 400 paths, and the second still did after the first was removed. `revive -h` states four flags and none is a cache; one run left the `TMPDIR` entry count unchanged.

    DESIGN. The rule runs the same `exported` rule with a plain `[rule.exported]` config and selects `.RuleName == "exported" and .Category == "naming"` in the pipe. That is ATTRIBUTION, not exemption, in the README's own words: the `comments` findings are owned and reported by `missing-docs-go`, which keeps `disableStutteringCheck` (checklist item 4). The two rules therefore partition revive's `exported` output between them with nothing owned twice and nothing dropped.

    Name: `stuttering-name-go`, named for its own concern, as `unused-dependencies-rust` is — no naming prompt rule ships that reads a `.go` file, so it supersedes nothing.
  timestamp: 2026-08-14T21:00:08.202170+00:00
- actor: claude-code
  id: 01m012fdez3gdvs4qfcwj4svtt
  text: |-
    RED→GREEN record, and the stale facts the card carried.

    RED. The rule file was moved aside and all 8 new acceptance tests failed with "`stuttering-name-go` must be a shipped tool rule for [\"go\"]". The rule was restored and all 8 passed.

    The Category filter was then MUTATED to `select(.RuleName == "exported")` — dropping `and .Category == "naming"` — and 7 of the 8 tests failed. The fixture pair is one of the 7: the pass fixture carries one exported name with NO doc comment on purpose, so a filter that stopped reading the category reports it and the pair fails.

    STALE FACTS THE CARD CARRIED, corrected against the current tree.
    1. "27 shipped rules match a `.go` file" — it was 25 before this card and is 26 now. Every place that stated 25 is corrected: `missing-docs-go.md` states 26 in two clauses of one sentence.
    2. `SHIPPED_RULES_THAT_READ_A_GO_FILE` moved to `.../tests/shipped/missing_docs.rs` and it is SORTED. `code-hygiene/stuttering-name-go` is inserted after `code-hygiene/no-commented-code`.
    3. `unused-code-go` is `dead-code-go` — the roster already carried the new name and needed no edit.

    COUNT CONSTANTS the new rule moved, each watched RED first:
    - `builtin/mod.rs` — a new `CODE_HYGIENE_NAMING_TOOL_RULES` group, chained into the roster, the count and the `supersedes` table (it supersedes nothing).
    - `scope_roster.rs` `SHIPPED_TOOL_RULE_COUNT` 25 → 26.
    - `zero_argument.rs` `FILES_SCOPE_RULE_COUNT` 14 → 15, and the "7 write the guard on the first line, and 7 under `set -e`" sentence is now 7 and 8.
    - `temp_directory.rs` `TEMP_DIRECTORY_RULE_COUNT` 17 → 18.
    - `crates/mirdan/src/builtin_validators.rs` `CODE_HYGIENE_FIXTURES` takes the two new fixture templates.

    CARDS ^1h52223 / ^mms9g8d. `mms9g8d`'s work was read. The rule drives revive DIRECTLY, so neither the per-workspace `GOLANGCI_LINT_CACHE` nor `allow-serial-runners` applies, and both halves are measured rather than assumed: 8 runs started together in one workspace each reported all 4 findings over 2 rounds, and a module of 400 packages took 0.12 s / 0.11 s / 0.12 s / 0.11 s over first-cold, first-warm, second-cold and second-warm at two paths, each reporting its own 400 paths. Two acceptance tests hold both halves. `function_length_go.rs`'s `rows_of_runs_started_together` was moved into `shipped.rs` and parameterized rather than copied, so the lock probe of both rules runs one helper.

    NOTHING under `crates/mirdan/retired-validators/` was touched.
  timestamp: 2026-08-14T21:22:32.031974+00:00
- actor: claude-code
  id: 01m012fnfr2tjnebwb349g87by
  text: |-
    ### implement — changed
    - evidence: 15 files. New — `builtin/validators/code-hygiene/rules/stuttering-name-go.md`, `builtin/validators/code-hygiene/fixtures/stuttering-name-go.{fail,pass}.go.tmpl`, `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/stuttering_name_go.rs`. Changed — `builtin/validators/code-hygiene/VALIDATOR.md`, `builtin/validators/code-hygiene/rules/missing-docs-go.md`, `crates/mirdan/src/builtin_validators.rs`, `crates/swissarmyhammer-validators/src/builtin/mod.rs`, and 6 files under `crates/swissarmyhammer-validators/src/review/tool_rules/tests/`. `cargo nextest run --workspace`: 14212 tests run, 14212 passed, 0 skipped. `cargo fmt --check` clean, `cargo clippy --workspace --all-targets -- -D warnings` clean.
    - next: /review
  timestamp: 2026-08-14T21:22:40.248604+00:00
- actor: claude-code
  id: 01m0131y6qapttktmk19z036af
  text: |-
    ### review — clean
    - evidence: `review sha 86ebb5eb6~1..86ebb5eb6` — 0 findings, 0 confirmed, 0 refuted, 18 validator-file pairs attempted, 0 failed. 10 files reviewed; the two `stuttering-name-go.*.go.tmpl` fixtures were excluded from scope as validator fixtures, so the demonstrated defect in them was not reported.
    - roster check: `stuttering-name-go` is in every roster that must carry it — `crates/swissarmyhammer-validators/src/builtin/mod.rs` (`CODE_HYGIENE_NAMING_TOOL_RULES`, chained into both the count assertion and the supersedes map), `crates/swissarmyhammer-validators/src/review/tool_rules/tests.rs` (`SHIPPED_STUTTERING_NAME_RULES`), `tests/shipped.rs` (`mod stuttering_name_go`), `tests/shipped/missing_docs.rs` (`SHIPPED_RULES_THAT_READ_A_GO_FILE`), and `crates/mirdan/src/builtin_validators.rs` (both fixtures).
    - count check: the Go-file roster reads 26 and is sorted, with the entry after `no-commented-code` and before `code-security/command-safety`. `SHIPPED_TOOL_RULE_COUNT` reads 26 with the 15 + 11 split. `builtin/validators/code-hygiene/rules/missing-docs-go.md` states 26 two times. `builtin/validators/code-hygiene/VALIDATOR.md` names the rule in its prose and its table but states no Go-file count, so it has none to hold. No stale 25 is left anywhere.
    - next: none. Task moves to done.
  timestamp: 2026-08-14T21:32:38.999063+00:00
- actor: claude-code
  id: 01m0133gmz53c1vg8t0pbr2vw8
  text: |-
    ### finish iteration 1 — clean. Card done.
    - implement: changed — 15 files, 4 of them new. A new shipped rule `code-hygiene/stuttering-name-go` owns the defect. It runs the SAME revive `exported` rule that missing-docs-go runs and selects `.RuleName == "exported" and .Category == "naming"`, so the two rules split revive's whole `exported` output with nothing owned twice and nothing dropped. `missing-docs-go` keeps `disableStutteringCheck` unchanged.
    - The filter reads the CATEGORY, not the message text — deliberately. revive's `sayRepetitiveInsteadOfStutters` rewrites "that stutters" as "that is repetitive" while the Category does not move, so a filter on the sentence breaks the moment that argument is set.
    - **The survey was run, not skipped**, each candidate over one probe file: of revive's 12 naming-category rules, `exported` reports the four repetitive names and `var-naming` reports only the underscore, the other ten silent; `staticcheck -checks all` reports ST1000, ST1003 and U1000 — it names more than `exported` does, but none of its checks reads a name against its package; `golangci-lint` with `default: all` is 115 linters and only revive reports the repetition.
    - **revive has neither golangci-lint hazard, measured** rather than assumed: eight runs started together in one workspace each reported all four findings over two rounds, and a module of 400 packages cost 0.12 / 0.11 / 0.12 / 0.11s over first-cold, first-warm, second-cold and second-warm at two paths, each reporting its own 400 paths. Two acceptance tests hold both halves.
    - **All three stale facts in the card were corrected**: the Go roster was 25 before this card rather than the stated 27, and is 26 now; the roster lives in `tests/shipped/missing_docs.rs` and is sorted, so the entry went in after `code-hygiene/no-commented-code`; and `dead-code-go` already carried its renamed form from ^n8ptdxb.
    - RED first: with the rule file moved aside all eight new acceptance tests failed; mutating the shipped filter to drop the Category clause failed seven of the eight, the fixture pair among them.
    - test: green — cargo nextest run --workspace 14212 passed, 0 failed, 0 skipped. fmt and clippy clean.
    - commit: 86ebb5eb6
    - review: clean — 0 findings, 18 pairs attempted, 0 failed, 0 skipped, 10 files reviewed, 2 not reviewed.

    **Roster consistency verified at every named line**, since adding a rule is the change most likely to leave one stale: `builtin/mod.rs:298` (chained into BOTH the total-count assertion and the supersedes map, with an empty supersedes slice because this rule replaces no prompt rule), `tests.rs:150`, `shipped.rs` module registration, `missing_docs.rs:421` with the sort holding at 26 entries, and mirdan's fixture roster. The count reads 26 everywhere it is stated — `scope_roster.rs:51` with the split updated to 15 files plus 11 workspace, and `missing-docs-go.md` twice, where the stale sentence "No shipped rule owns a stuttering Go name today" is replaced by one naming the new owner. A repository sweep for a stale 25 found none. VALIDATOR.md names the rule in four places and states no count, so it has none to hold.

    **The reviewer caught a false premise in its OWN work** and did not record it: an `rg` sweep rendered the rule name as `n-name-go`, which read as a broken registration. Re-reading the named lines with the file tool showed every one reads `stuttering-name-go` — a shell rendering artifact, not file content. The two fixtures were excluded from scope by the engine itself as `validator fixture`, so the deliberate defect the fail fixture demonstrates was never a candidate finding.
  timestamp: 2026-08-14T21:33:30.655615+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffffff8580
title: No shipped rule owns a stuttering Go name
---
`revive` reports a stuttering exported Go name — a name that repeats its own
package name, so a caller writes the word two times: `staged.StagedType`. The
`missing-docs-go` rule turns that check off with `disableStutteringCheck`,
because the rule supersedes `missing-docs` and no other, and a stuttering name
is not a missing doc comment.

`code-hygiene/stuttering-name-go` now owns the defect. It runs the same revive
`exported` rule with a plain `[rule.exported]` config and selects
`.RuleName == "exported" and .Category == "naming"`, so the two rules together
are revive's whole `exported` output with no finding owned two times and none
dropped.

## What a rule needs

- [x] Decide the tool. `revive` states a stuttering finding under
      `RuleName: exported` with `Category: naming`, and states a documentation
      finding under the SAME rule name with `Category: comments`. Measured on
      revive 1.15.0. A Go naming rule can therefore run the same `exported`
      rule and select the `naming` category.
- [x] Read the message form before a filter reads it. The default message is
      `type name will be used as staged.StagedType by other packages, and that
      stutters; consider calling this Type`. The `sayRepetitiveInsteadOfStutters`
      argument writes `that is repetitive` in place of `that stutters`, so a
      filter on the word alone breaks when the argument is set. The `Category`
      field does not move. Both forms are measured on revive 1.15.0. The shipped
      filter reads `Category`, and the acceptance test holds each finding to the
      QUALIFIED NAME the message carries, which does not move either.
- [x] Survey the other Go naming tools before the rule is written. `exported`
      holds the stutter check alone, and `staticcheck` names more. Measured over
      one probe file: revive's other 11 `naming`-category rules are silent but
      `var-naming`, which reports the UNDERSCORE alone; `staticcheck -checks all`
      reports ST1000, ST1003 and U1000; `golangci-lint` with `default: all`
      (115 linters) reports the stutter through `revive` and nothing else. The
      survey is in `builtin/validators/code-hygiene/VALIDATOR.md` and in the
      rule body.
- [x] Keep `disableStutteringCheck` in `missing-docs-go`. That rule supersedes
      `missing-docs` alone, so it must not report a name. It is unchanged.
- [x] Ship a fixture pair and an acceptance test through the real tool. Eight
      acceptance tests in
      `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/stuttering_name_go.rs`,
      each watched RED before the rule shipped.
- [x] Correct the sentence in
      `builtin/validators/code-hygiene/rules/missing-docs-go.md` that states no
      rule owns the defect, and correct
      `SHIPPED_RULES_THAT_READ_A_GO_FILE` in
      `crates/swissarmyhammer-validators/src/review/tool_rules/tests/shipped/missing_docs.rs`.
      The roster is 26 now, and the rule body states 26.

Found on ^s2056e1. #tool-validators #objectivity