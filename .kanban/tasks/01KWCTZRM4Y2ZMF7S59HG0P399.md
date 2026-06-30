---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kwcvbwxa77f1pjhwe31555j6
  text: |-
    Implemented Swift project detection end-to-end.

    Files changed:
    - crates/swissarmyhammer-project-detection/src/types.rs: ProjectType::Swift variant (serde "swift"), marker_files [Package.swift, *.xcodeproj, *.xcworkspace], ProjectSymbols.swift field + get() arm; updated 3 enumerated tests.
    - crates/swissarmyhammer-project-detection/src/detect.rs: added Swift to detect_project_at_path priority list.
    - crates/swissarmyhammer-tools/src/mcp/tools/code_context/detect.rs: project_type_name=>"Swift", project_type_key=>"swift", partial_name_for_type=>Some("_partials/project-types/swift"); added Swift to test_project_type_name_all_types, test_all_project_types_have_renderable_guidelines, test_project_type_key_all_types.
    - crates/swissarmyhammer-lsp/src/registry.rs: added Swift to test_all_project_types_queryable.
    - builtin/lsp/sourcekit-lsp.yaml: project_types [] -> [swift].
    - builtin/project-detection/config.yaml: added swift glyph (reused sourcekit-lsp icon U+E699).
    - builtin/_partials/project-types/swift.md: new partial covering SwiftPM + Xcode, prefers ULID via yaslab/ULID.swift (Package.swift dep entry, product ULID, `let id = ULID()`).

    Verification (all green):
    - cargo nextest run -p swissarmyhammer-project-detection: 31/31 passed
    - cargo nextest run -p swissarmyhammer-lsp: 223/223 passed
    - cargo nextest run -p swissarmyhammer-tools code_context::detect: 26/26 passed (incl. renderable-guidelines test rendering swift partial through Liquid)
    - cargo fmt: clean (reverted unrelated formatting drift in review/validators files I did not touch)
    - cargo clippy --workspace --all-targets -- -D warnings: exit 0, no warnings
  timestamp: 2026-06-30T18:06:22.634599+00:00
- actor: claude-code
  id: 01kwcvf0k54vaq7rnqg63t7zp0
  text: 'Implement landed green in `doing`. 6 files changed + new partial `builtin/_partials/project-types/swift.md`. Verified: project-detection 31/31, lsp 223/223, tools code_context::detect 26/26 (swift partial renders through Liquid), fmt clean, clippy -D warnings clean, adversarial double-check PASS. config.yaml swift glyph is byte-identical to sourcekit-lsp.yaml icon (U+E699). sourcekit-lsp.yaml wired project_types: [swift]. Next: full /test → checkpoint commit → /review HEAD~1..HEAD.'
  timestamp: 2026-06-30T18:08:04.709915+00:00
- actor: claude-code
  id: 01kwcwm07r0jevrmne24ggkapt
  text: 'Review iteration 1: 21 production findings recorded, task in `review`. Reviewer caveat: it read the kanban task file (not source `+` hunks) as the semantic diff, so its ADDED-vs-PRE-EXISTING split is INFERRED — implementer must verify each finding against real source before acting. Theme: the bulk are the 5 parallel `match ProjectType` dispatches (types.rs get()/marker_files(); code_context/detect.rs name/key/partial) flagged for table-driven extraction — this directly matches the user''s "needless duplication of project type detection" concern, so the data-driven refactor is in-scope and desirable. Looping back to /implement to address the checklist at the root. Guardrail: iteration 1, nothing repeated yet.'
  timestamp: 2026-06-30T18:28:16.760622+00:00
- actor: claude-code
  id: 01kwcx29295dbdt7csfs6jy0tf
  text: |-
    Re-implement iteration 2: worked all 21 review findings at the root after verifying each against actual source (reviewer caveat noted: line numbers were stale but every finding was real).

    Findings 1 (table-driven dispatch): types.rs get()/marker_files() now lookup a single const PROJECT_TYPE_SPECS table via spec_for() (struct: project_type, marker_files, symbol fn-ptr). code_context/detect.rs project_type_name/key/partial now lookup a single const PROJECT_TYPE_DATA table via project_type_data(). Added completeness tests + a serde-rename-coupling guard test (test_project_type_key_matches_serde serializes each variant and asserts key == serde repr).
    Finding 2 (doc comments): all ProjectSymbols fields documented (rust..swift, incl. c_cpp/dart the reviewer omitted).
    Finding 3 (error handling): detect.rs detect_projects + all private helpers now return thiserror ProjectDetectionError { Canonicalize, ReadFile }; exported from lib.rs; added thiserror dep. Display preserves "Failed to canonicalize root path". canonicalize-error test now also matches! the typed variant.
    Finding 4 (constants): RUST_ANALYZER_STARTUP_TIMEOUT_SECS=30 + HEALTH_CHECK_INTERVAL_SECS=60 in registry.rs; DEFAULT_DETECT_MAX_DEPTH=3 in code_context/detect.rs.
    Finding 5 (nesting): extract_toml_array flattened into split_open_line + consume_array_line + push_cleaned_items (behavior identical, all edge-case tests green). format_detected_projects extracts format_workspace_info (4 levels -> 2).
    Finding 6 (dead code): removed should_stop_after_project + its always-false if; replaced with a comment documenting that traversal never stops early (monorepo behavior preserved).
    Finding 7 (ergonomics): resolve_workspace_path takes Option<&str>; call site uses request.path.as_deref().

    Verification (all green): cargo fmt clean; cargo nextest -p swissarmyhammer-project-detection -p swissarmyhammer-lsp = 255/255; cargo nextest -p swissarmyhammer-tools code_context::detect = 28/28 (incl. new guard tests + swift partial renders through Liquid); cargo clippy --workspace --all-targets -- -D warnings = exit 0, zero warnings. No finding contradicted another. Adversarial double-check running.
  timestamp: 2026-06-30T18:36:04.553370+00:00
- actor: claude-code
  id: 01kwcx7z96vzv48v1k2rfdtj3c
  text: 'Iteration 2 implement: all 21 findings worked at root, all checked. Highlights: introduced const PROJECT_TYPE_SPECS (types.rs) + PROJECT_TYPE_DATA (tools detect.rs) table-driven dispatch — collapses the 5 parallel match ProjectType statements into single spec tables + thin lookups (future project type = one table entry; directly resolves the duplication concern). Added ProjectDetectionError (thiserror) replacing Result<_,String>. Named constants for timeouts + max_depth. Flattened extract_toml_array + format_detected_projects nesting. Removed dead should_stop_after_project. Option<&String>→Option<&str>. New guard tests: every_variant_has_a_spec_entry, test_every_project_type_has_data, test_project_type_key_matches_serde. Verified: project-detection+lsp 255/255, tools code_context::detect 28/28, clippy --workspace -D warnings clean, double-check PASS. 7 files (incl. lib.rs, Cargo.toml, Cargo.lock). Next: full /test → checkpoint commit → re-review.'
  timestamp: 2026-06-30T18:39:11.142119+00:00
- actor: claude-code
  id: 01kwczszd07ztfpr7yt8tt554d
  text: 'Iteration 2 test: GREEN. Affected crate + ALL real detect_projects callers (project-detection, tools, lsp, cli, config) = 3051/3051 passed. Confirms the Result<_,String>→ProjectDetectionError typed-error change compiles+passes across every caller (doctor/checks.rs, config/template_context.rs, lsp/supervisor.rs, tools code_context detect/doctor/mod). Full rdeps sweep: 10824 passed, 5 timeouts — all heavy real-model/LLM e2e (kanban-app ai_panel_e2e qwen, llama-agent agent_tools_mount/dual_source, swissarmyhammer-agent review_real_model_e2e); none reference detect_projects in source = pre-existing model-e2e flakiness (known qwen NoKvCacheSlot KV-cache issue), NOT a regression from this task. Proceeding to checkpoint commit → re-review.'
  timestamp: 2026-06-30T19:23:58.240824+00:00
- actor: claude-code
  id: 01kwd0fzrzbshcjtt5qxm41wka
  text: |-
    Review iteration 2: all 21 prior findings verified [x]. 3 NEW findings (not repeats; guardrail not triggered):
    1. detect.rs:16 — thiserror message capitalized; make lowercase ("failed to canonicalize root path: {0}").
    2. detect.rs:110 — hardcoded ProjectType array in detect_project_at_path duplicates PROJECT_TYPE_SPECS roster (same crate); iterate the spec instead.
    3. tools code_context/detect.rs:73 — PROJECT_TYPE_DATA is a THIRD parallel variant roster duplicating PROJECT_TYPE_SPECS; consolidate.
    Theme: iter-2 collapsed dispatch logic but relocated the roster-drift risk into 3 parallel lists. Root fix = ONE authoritative roster. Precedent: ProjectSymbols glyphs already put per-type presentation metadata in the detection crate, so extending PROJECT_TYPE_SPECS with name/key/partial fields is consistent, NOT a layering violation. Looping to /implement to make PROJECT_TYPE_SPECS the single source everything derives from.
  timestamp: 2026-06-30T19:35:59.519181+00:00
- actor: claude-code
  id: 01kwd0y24r3xj1vxmg4ghjckpe
  text: |-
    Re-implement iteration 3: worked all 3 review findings (2026-06-30 14:24) at the root after verifying each against actual source. Theme was correct — iter-2 collapsed dispatch but left THREE parallel rosters; this collapses them to ONE.

    Finding 1 (detect.rs error messages): lowercased both thiserror variants — Canonicalize ("failed to canonicalize root path: {0}") and ReadFile ("failed to read {path}: {source}"). Updated test_detect_projects_canonicalize_error substring to "failed to canonicalize root path"; test stays green + still matches!s the typed variant.

    Finding 2 (detect_project_at_path hardcoded array): deleted the 12-entry array; now iterates project_type_specs() (the authoritative PROJECT_TYPE_SPECS roster). Reordered PROJECT_TYPE_SPECS so its table order IS the detection priority order — Go before Python to byte-match the prior array (NOT Python-before-Go as the task text recalled; double-check confirmed the real prior code was Go-first, so behavior is unchanged). Documented the table order = priority order.

    Finding 3 (PROJECT_TYPE_DATA third roster): ELIMINATED PROJECT_TYPE_DATA + ProjectTypeData + project_type_data from tools detect.rs. Extended ProjectTypeSpec (project-detection types.rs) with name/key/partial fields — consistent with the crate already owning ProjectSymbols glyphs (NOT a layering violation). project_type_name/project_type_key/partial_name_for_type are now thin spec_for(pt) lookups. Exported project_type_specs(), spec_for, ProjectTypeSpec from lib.rs. partial strings byte-identical (_partials/project-types/{key}, Php None). Guard tests: moved serde-key guard to types.rs (spec_key_matches_serde_repr) + added spec_partial_matches_key; tools-layer test_project_type_key_matches_serde + test_every_project_type_has_data rewritten to iterate project_type_specs() (non-vacuous).

    Single authoritative roster confirmed: grepped non-test code — PROJECT_TYPE_SPECS is the ONLY hand-maintained full-variant roster. All other full-variant enumerations (registry.rs, types.rs tests, tools detect.rs tests) are inside #[cfg(test)], which the task explicitly permits as guards. Adding a project type now = one PROJECT_TYPE_SPECS entry.

    Verification (all green, run this message): cargo fmt clean; cargo nextest -p swissarmyhammer-project-detection -p swissarmyhammer-tools -p swissarmyhammer-lsp -p swissarmyhammer-cli -p swissarmyhammer-config = 3053/3053 passed; cargo clippy --workspace --all-targets -D warnings = exit 0 zero warnings. Adversarial double-check = PASS (verified single roster, byte-identical order/keys/names/partials, non-vacuous guards, no layering concern, no dangling refs to deleted structs).

    Files: crates/swissarmyhammer-project-detection/src/{types.rs,detect.rs,lib.rs}, crates/swissarmyhammer-tools/src/mcp/tools/code_context/detect.rs. Left green in doing for /review. (Note: parallel-shell modifications to review/validators files + task 2v84v in git status are NOT mine — untouched.)
  timestamp: 2026-06-30T19:43:40.696774+00:00
- actor: claude-code
  id: 01kwd0z95vqqnb283wmdakt302
  text: 'Iteration 3 implement: all 3 findings fixed at root. (1) Both ProjectDetectionError messages lowercased + test substring updated. (2) detect_project_at_path now iterates project_type_specs() — hardcoded array deleted; SPECS reordered to byte-match prior detection order (Go before Python, verified vs real prior code). (3) PROJECT_TYPE_DATA/ProjectTypeData/project_type_data deleted; ProjectTypeSpec gained name/key/partial; tools project_type_name/key/partial_name_for_type are thin spec_for() lookups; exported project_type_specs/spec_for/ProjectTypeSpec from lib.rs. Partial strings byte-identical, PHP None. Guards kept non-vacuous (spec_key_matches_serde_repr, spec_partial_matches_key). Single authoritative roster confirmed by grep — only PROJECT_TYPE_SPECS in non-test code; future project type = 1 table entry. Verified: affected crates + callers 3053/3053, clippy --workspace -D warnings clean, double-check PASS. Test evidence = implementer''s scoped run this session (avoided the slow rdeps sweep w/ known model-e2e timeouts). Proceeding to checkpoint commit. NB: tree has unrelated parallel-shell edits (review/validators, task 2v84v) — commit must stay scoped to this task''s files.'
  timestamp: 2026-06-30T19:44:20.667599+00:00
- actor: claude-code
  id: 01kwd1npktaqtmwjgwgknac75c
  text: |-
    Review iteration 3: prior 3 findings verified [x] (roster consolidation + lowercase errors held). 2 NEW production findings (7 test-code findings dropped under blanket test-refactoring exception; distinct from all prior — guardrail not triggered; review converging 21→3→2):
    1. types.rs — ProjectTypeSpec is now pub but lacks #[derive(Debug)]; add it (fn-ptr field supports Debug/Clone/Copy).
    2. detect.rs — detect_rust_workspace + detect_npm_workspace duplicate the read_to_string + ProjectDetectionError::ReadFile mapping; extract a shared read_optional_config helper.
    Both clean, small, non-contradictory. Looping to /implement.
  timestamp: 2026-06-30T19:56:35.322726+00:00
- actor: claude-code
  id: 01kwd1vv0p6shxjqjc3rerhypf
  text: |-
    Worked the 2026-06-30 14:45 review findings (pulled review→doing).

    Finding 1 (types.rs): Added `#[derive(Debug, Clone, Copy)]` to `pub struct ProjectTypeSpec`. Debug is the required derive (public-type contract); Clone+Copy added for consistency with the crate's other Copy spec type `ProjectType` and because every field is Copy (ProjectType is Copy, &'static refs, fn pointer, Option<&'static str>). Compiles, clippy clean.

    Finding 2 (detect.rs): Extracted `read_optional_config(path: &Path, filename: &str) -> Result<Option<String>, ProjectDetectionError>` — joins filename, returns Ok(None) when absent, reads with the identical ProjectDetectionError::ReadFile { path, source } mapping otherwise. Both detect_rust_workspace ("Cargo.toml") and detect_npm_workspace ("package.json") now call it via `match ... { Some(c) => c, None => return Ok(None) }`. Exact behavior preserved: same return values, same error variant/fields, same file-absent→Ok(None) semantics. The npm-specific `package_json_path` local was removed in favor of the shared path inside the helper.

    Verification (all green):
    - cargo fmt -p swissarmyhammer-project-detection -- --check → exit 0
    - cargo clippy --workspace --all-targets -- -D warnings → exit 0
    - cargo nextest run -p swissarmyhammer-project-detection -p swissarmyhammer-tools -p swissarmyhammer-lsp -p swissarmyhammer-cli -p swissarmyhammer-config → 3053 tests run, 3053 passed, 0 failed. All workspace-detection tests (test_detect_rust_project, test_detect_monorepo, test_detect_npm_workspace_{array,string,absent_key,no_package_json,object}_form) pass.

    Left GREEN in doing for /review. Did not touch unrelated parallel-shell changes.
  timestamp: 2026-06-30T19:59:56.438620+00:00
- actor: claude-code
  id: 01kwd1whaspaahby3jtwm1fx9m
  text: 'Iteration 4 implement: both findings resolved. (1) types.rs ProjectTypeSpec gained #[derive(Debug, Clone, Copy)] (all fields Copy). (2) detect.rs extracted read_optional_config(path, filename) -> Result<Option<String>, ProjectDetectionError>; both detect_rust_workspace + detect_npm_workspace call it; behavior byte-for-byte preserved. Verified: affected crates + callers 3053/3053, clippy --workspace -D warnings clean, fmt clean. Parallel-shell files untouched. Proceeding to checkpoint commit → re-review.'
  timestamp: 2026-06-30T20:00:19.289854+00:00
- actor: claude-code
  id: 01kwd2a6q96qmragxpnekrwq4g
  text: 'Review iteration 4: both prior findings verified [x]. 1 NEW finding (convergence 21→3→2→1): types.rs:72 ProjectSymbols::default() can panic on malformed builtin config YAML (.expect) but lacks a # Panics doc. This is pre-existing code in a delta-touched file (in scope). To prevent a per-nit file-walk next round, instructing /implement to fix the CAUSE across the whole file: add # Panics to default() AND sweep types.rs + detect.rs for every other public-API panic path (.expect/.unwrap/panic!/indexing) and document each — so re-review finds zero recurrences. Guardrail: 4 review rounds, each distinct + reducing — not tripped; watching for non-convergence after this pass.'
  timestamp: 2026-06-30T20:07:47.177784+00:00
- actor: claude-code
  id: 01kwd2frgd0fcfjk7hmrt1cmt9
  text: |-
    Worked 2026-06-30 15:01 review finding (panic doc). Doc-comment-only pass, no behavior change.

    Cause-sweep of both task files for every public API that can panic:
    - types.rs `ProjectSymbols::default()` — added # Panics (malformed BUILTIN_CONFIG_YAML deserialize). [cited finding]
    - types.rs `spec_for()` — added # Panics (.expect if a ProjectType has no PROJECT_TYPE_SPECS entry).
    - types.rs `ProjectSymbols::get()` — added # Panics (delegates to spec_for).
    - types.rs `ProjectType::marker_files()` — added # Panics (delegates to spec_for).
    - detect.rs: only public fn is `detect_projects`, returns Result, canonicalize errors → Err; no .expect/.unwrap/panic!/indexing. Cannot panic — deliberately NOT given a # Panics (would be misleading). All other detect.rs fns are private.
    - Other types.rs public items (project_type_specs, should_skip_directory, data structs/enums/consts) cannot panic.

    Verification (all green):
    - cargo fmt -p swissarmyhammer-project-detection -- --check: exit 0
    - cargo clippy --workspace --all-targets -- -D warnings: exit 0
    - cargo nextest run -p swissarmyhammer-project-detection -p swissarmyhammer-tools -p swissarmyhammer-lsp -p swissarmyhammer-cli -p swissarmyhammer-config: 3053 passed, 0 failed, 0 skipped.

    Task left GREEN in doing for /review.
  timestamp: 2026-06-30T20:10:49.229246+00:00
- actor: claude-code
  id: 01kwd2gj4ahwwgb87n6g1webzv
  text: 'Iteration 5 implement: finding fixed + cause swept across both files. # Panics docs added to all 4 panicking public APIs in types.rs: default() (malformed YAML .expect), spec_for() (.expect missing variant), get() + marker_files() (delegate to spec_for). detect.rs detect_projects() correctly NOT documented (returns Result, no panic path — avoided misleading doc). Doc-only, no behavior change. Verified: affected crates + callers 3053/3053, clippy --workspace clean, fmt clean. Only types.rs changed. Proceeding to checkpoint commit → re-review. Expecting clean (panic-doc cause eliminated file-wide).'
  timestamp: 2026-06-30T20:11:15.466450+00:00
position_column: doing
position_ordinal: '8380'
title: Add Swift project detection + project-types partial (prefer ULID via yaslab/ULID.swift)
---
Add first-class Swift support to project detection and create the matching `swift` project-types partial. The partial filename MUST match the detection key string (`project_type_key` == "swift" → `builtin/_partials/project-types/swift.md`), exactly as `flutter`, `java-gradle`, etc. do today.

## Architecture note — there is NO duplicate detection
Both the code-context `detect projects` op AND the LSP registry share ONE `ProjectType` enum (`swissarmyhammer-project-detection`; `swissarmyhammer-lsp/src/registry.rs:9` and `types.rs:14` import it). The thing that looks like duplication is actually an INCOMPLETE enum: Swift has no `ProjectType` variant, so `builtin/lsp/sourcekit-lsp.yaml` declares `project_types: []` and the Swift LSP is routed only by `file_extensions: [swift]` (the extension-fallback path `servers_for_extensions`). Kotlin and Ruby are in the same boat (also `project_types: []`) — but those are OUT OF SCOPE here. Adding `ProjectType::Swift` to the single enum fixes both layers at once: code-context detects Swift projects, and the LSP can be routed by project type.

## Review Findings (2026-06-30 14:24)

- [x] `crates/swissarmyhammer-project-detection/src/detect.rs:16` — Error message lowercased: both `Canonicalize` (`"failed to canonicalize root path: {0}"`) and `ReadFile` (`"failed to read {path}: {source}"`). The canonicalize-error test substring updated to `"failed to canonicalize root path"`; test stays green.
- [x] `crates/swissarmyhammer-project-detection/src/detect.rs:110` — `detect_project_at_path` now iterates `project_type_specs()` (the authoritative `PROJECT_TYPE_SPECS` roster) instead of a hardcoded array; the array is gone. `PROJECT_TYPE_SPECS` reordered so its table order IS the detection priority order (Go before Python to match the prior list), documented as such.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/detect.rs:73` — `PROJECT_TYPE_DATA`/`ProjectTypeData`/`project_type_data` ELIMINATED. `ProjectTypeSpec` (project-detection crate) extended with `name`/`key`/`partial` fields; tools-layer `project_type_name`/`project_type_key`/`partial_name_for_type` are now thin lookups into `spec_for(pt)`. The serde-key guard lives in types.rs (`spec_key_matches_serde_repr`) AND is mirrored at the tools layer (`test_project_type_key_matches_serde` iterates `project_type_specs()`). `partial` strings unchanged (`_partials/project-types/{key}`, PHP None); added `spec_partial_matches_key` guard.

Result: `PROJECT_TYPE_SPECS` is the ONE authoritative per-variant roster. Adding a project type = one table entry. No parallel full-variant list remains in non-test code (verified by grep); the remaining full-variant enumerations are all test guards, which the task explicitly permits.

## Review Findings (2026-06-30 14:45)

- [x] `crates/swissarmyhammer-project-detection/src/types.rs` — Public struct `ProjectTypeSpec` lacks a `Debug` implementation. This delta made the struct `pub` (and added `name`/`key`/`partial` fields), so the public-type contract now applies: all public types with a non-empty representation must derive `Debug`. Add `#[derive(Debug)]` to `ProjectTypeSpec`.
- [x] `crates/swissarmyhammer-project-detection/src/detect.rs` — `detect_rust_workspace` and `detect_npm_workspace` duplicate the same file-reading + error-handling pattern: join a filename to the path, check existence, `fs::read_to_string` with the identical `ProjectDetectionError::ReadFile { path, source }` mapping, returning `Ok(None)` when the file is absent. Extract a shared helper (e.g. `fn read_optional_config(path: &Path, filename: &str) -> Result<Option<String>, ProjectDetectionError>`) that encapsulates the existence check, read, and error mapping, then call it from both functions so the two cannot drift.

## Review Findings (2026-06-30 15:01)

- [x] `crates/swissarmyhammer-project-detection/src/types.rs:72` — The `default()` method can panic if the builtin config YAML is malformed, but this panic is not documented in the doc comment, violating the rule that panics must be documented. Added a `# Panics` section. CAUSE-SWEEP of both task files for every public API that can panic: in types.rs added/extended `# Panics` on `ProjectSymbols::default()` (YAML parse), `spec_for()` (`.expect` if a variant has no spec entry), `ProjectSymbols::get()` and `ProjectType::marker_files()` (both delegate to `spec_for`). detect.rs has only one public fn, `detect_projects`, which returns `Result` (canonicalize errors → `Err`) and cannot panic — no misleading `# Panics` added. Doc-only; no behavior change.