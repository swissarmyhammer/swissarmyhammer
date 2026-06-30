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