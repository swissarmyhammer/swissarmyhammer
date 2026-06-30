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
position_column: doing
position_ordinal: '8480'
title: Add Swift project detection + project-types partial (prefer ULID via yaslab/ULID.swift)
---
Add first-class Swift support to project detection and create the matching `swift` project-types partial. The partial filename MUST match the detection key string (`project_type_key` == "swift" → `builtin/_partials/project-types/swift.md`), exactly as `flutter`, `java-gradle`, etc. do today.

## Architecture note — there is NO duplicate detection
Both the code-context `detect projects` op AND the LSP registry share ONE `ProjectType` enum (`swissarmyhammer-project-detection`; `swissarmyhammer-lsp/src/registry.rs:9` and `types.rs:14` import it). The thing that looks like duplication is actually an INCOMPLETE enum: Swift has no `ProjectType` variant, so `builtin/lsp/sourcekit-lsp.yaml` declares `project_types: []` and the Swift LSP is routed only by `file_extensions: [swift]` (the extension-fallback path `servers_for_extensions`). Kotlin and Ruby are in the same boat (also `project_types: []`) — but those are OUT OF SCOPE here. Adding `ProjectType::Swift` to the single enum fixes both layers at once: code-context detects Swift projects, and the LSP can be routed by project type.

## Background / how the pieces wire together
- `crates/swissarmyhammer-project-detection/src/types.rs`: `ProjectType` enum (serde `rename_all = "lowercase"`), `ProjectSymbols` struct + `get()`, `marker_files()`, and tests that enumerate every variant.
- `crates/swissarmyhammer-project-detection/src/detect.rs`: `detect_project_at_path` priority list (must include the new variant or it is never detected).
- `crates/swissarmyhammer-tools/src/mcp/tools/code_context/detect.rs`: `project_type_name` (display), `project_type_key` (stable key, matches serde rename + partial filename), `partial_name_for_type` (`Some("_partials/project-types/{key}")`). Tests `test_all_project_types_have_renderable_guidelines`, `test_project_type_name_all_types`, `test_project_type_key_all_types` enumerate every variant and render each partial through Liquid — they will fail until the new variant is added everywhere AND the partial file exists and renders non-empty.
- `builtin/project-detection/config.yaml`: `symbols:` map; `ProjectSymbols::default()` parses this and will fail if a new struct field has no corresponding yaml key.
- `crates/swissarmyhammer-lsp/src/registry.rs`: `test_all_project_types_queryable` enumerates variants — add Swift there too.
- `builtin/lsp/sourcekit-lsp.yaml`: currently `project_types: []`.

## Work items

### 1. types.rs (project-detection)
- Add `Swift` variant to `ProjectType` (lowercase serde => "swift").
- Add `marker_files()` arm: `&["Package.swift", "*.xcodeproj", "*.xcworkspace"]` (Package.swift = SwiftPM; xcodeproj/xcworkspace = Xcode).
- Add `swift: String` field to `ProjectSymbols`; add `ProjectType::Swift => &self.swift` to `get()`.
- Update the variant lists / symbol assertions in the types.rs tests (`project_symbols_default_loads_successfully`, `project_symbols_get_returns_nonempty_for_all_variants`, `project_symbols_get_maps_variants_to_correct_fields`).

### 2. config.yaml
- Add `swift: "<glyph> "` to the `symbols:` map (Nerd Font swift glyph — reuse the same glyph as `builtin/lsp/sourcekit-lsp.yaml`'s `icon` for consistency).

### 3. detect.rs (project-detection crate)
- Add `ProjectType::Swift` to the `project_types` array in `detect_project_at_path`.

### 4. code_context/detect.rs (tools crate)
- `project_type_name`: `ProjectType::Swift => "Swift"`.
- `project_type_key`: `ProjectType::Swift => "swift"`.
- `partial_name_for_type`: `ProjectType::Swift => Some("_partials/project-types/swift")`.
- Add `Swift` to the variant lists in `test_project_type_name_all_types`, `test_all_project_types_have_renderable_guidelines`, and `test_project_type_key_all_types` (expected key "swift").

### 5. LSP wiring
- `crates/swissarmyhammer-lsp/src/registry.rs`: add `ProjectType::Swift` to the loop in `test_all_project_types_queryable`.
- `builtin/lsp/sourcekit-lsp.yaml`: change `project_types: []` → `project_types:\n  - swift` so the Swift LSP routes by project type (not extension-only). Confirm the YAML still loads (LSP_REGISTRY tests).

### 6. New partial: builtin/_partials/project-types/swift.md
Follow the exact format of the existing partials (frontmatter `title`/`description`/`partial: true`, then `### Swift Project Guidelines`, testing section, common commands, file locations). Cover both SwiftPM and Xcode flows. Include the project default convention:

**Prefer ULID for unique identifiers** as the default — use https://github.com/yaslab/ULID.swift (SwiftPM package URL `https://github.com/yaslab/ULID.swift`, product `ULID`). Show adding it to `Package.swift` dependencies and a one-line usage example (`let id = ULID()`), and state ULID is preferred over UUID for new identifiers.

Suggested command coverage:
- Testing (do NOT glob; `swift test` discovers tests; Xcode uses `xcodebuild test`): `swift test`, `swift test --filter <Suite>/<test>`.
- Build/run: `swift build`, `swift run`.
- Format/lint: `swift format` (or `swiftformat`), `swiftlint` if present.
- Deps: edit `Package.swift`; `swift package resolve` / `swift package update`.
- File locations: `Sources/`, `Tests/`, `Package.swift`; git-ignored `.build/`.

## Verification
- `cargo nextest run -p swissarmyhammer-project-detection`
- `cargo nextest run -p swissarmyhammer-lsp`
- `cargo nextest run -p swissarmyhammer-tools` (detect tests — confirms the partial renders non-empty and frontmatter is stripped).
- `cargo fmt` + `cargo clippy -- -D warnings`.

## Out of scope
- Kotlin / Ruby (same empty-`project_types` situation) — separate follow-up.

## Review Findings (2026-06-30 13:11)

- [x] `crates/swissarmyhammer-lsp/src/registry.rs:22` — Hardcoded timeout value 30 (seconds) for startup timeout should be a named constant to explain its purpose and allow adjustment. Define a constant like `const RUST_ANALYZER_STARTUP_TIMEOUT_SECS: u64 = 30;` and use it here.
- [x] `crates/swissarmyhammer-project-detection/src/detect.rs:8` — Public library function returns `Result<Vec<DetectedProject>, String>` instead of a typed error enum. This prevents callers from matching on specific error cases and is worse than `anyhow::Error` — returning an opaque string message gives callers no structured error information to work with. Define a typed error enum using `thiserror` (e.g., `#[derive(thiserror::Error)]` with variants like `#[error("Failed to canonicalize root path: {0}")]` and `#[error("Failed to read directory: {0}")]`), and return `Result<Vec<DetectedProject>, ProjectDetectionError>` instead.
- [x] `crates/swissarmyhammer-project-detection/src/detect.rs:235` — The `extract_toml_array` function has excessive nesting depth reaching 6 levels (for → else if → if → if let → for → if let), making the control flow hard to follow. The function implements a state machine to parse TOML array syntax with multiple edge cases (arrays spanning multiple lines, items with closing brackets, etc.), but the nested conditionals obscure the logic. Refactor into separate helper functions: `extract_toml_array_line` (handles parsing a single line within an array), `should_parse_multiline_open` (detects array start), `should_parse_multiline_close` (detects array end). Extract the pattern-matching logic for inline arrays (with the `let (items_part, closed) = if ... else if ... else` block) into a dedicated `parse_inline_array` function to reduce nesting in the main loop.
- [x] `crates/swissarmyhammer-project-detection/src/detect.rs:291` — Needless helper that wraps a single call site and always returns a constant. The function `should_stop_after_project()` has exactly one caller (line 52–54 in `detect_projects_recursive`) and unconditionally returns `false`, making the `if should_stop` control flow at line 55 dead code. Wrapping a constant in a function adds no meaningful abstraction. Remove the `should_stop_after_project()` function and the dead `if should_stop` block. Replace with a comment explaining that traversal never stops early to find all nested projects in monorepos.
- [x] `crates/swissarmyhammer-project-detection/src/types.rs:73` — Public struct field `rust` lacks documentation. Other public structs in this file (DetectedProject, WorkspaceInfo) consistently document each field individually. Add a doc comment: `/// Nerd Font symbol for Rust projects`.
- [x] `crates/swissarmyhammer-project-detection/src/types.rs:74` — Public struct field `nodejs` lacks documentation, inconsistent with the documented fields in other structs in this file. Add a doc comment: `/// Nerd Font symbol for Node.js projects`.
- [x] `crates/swissarmyhammer-project-detection/src/types.rs:75` — Public struct field `python` lacks documentation. Add a doc comment: `/// Nerd Font symbol for Python projects`.
- [x] `crates/swissarmyhammer-project-detection/src/types.rs:76` — Public struct field `go` lacks documentation. Add a doc comment: `/// Nerd Font symbol for Go projects`.
- [x] `crates/swissarmyhammer-project-detection/src/types.rs:77` — Public struct field `java` lacks documentation. Add a doc comment: `/// Nerd Font symbol for Java projects`.
- [x] `crates/swissarmyhammer-project-detection/src/types.rs:78` — Public struct field `csharp` lacks documentation. Add a doc comment: `/// Nerd Font symbol for C# / .NET projects`.
- [x] `crates/swissarmyhammer-project-detection/src/types.rs:81` — Public struct field `php` lacks documentation. Add a doc comment: `/// Nerd Font symbol for PHP projects`.
- [x] `crates/swissarmyhammer-project-detection/src/types.rs:82` — Public struct field `swift` lacks documentation. Add a doc comment: `/// Nerd Font symbol for Swift projects`.
- [x] `crates/swissarmyhammer-project-detection/src/types.rs:89` — ProjectSymbols::get() is a match statement that maps each ProjectType variant to a struct field. Every new ProjectType requires manually adding an arm here, risking drift and omission. Extract the ProjectType-to-symbol-field mapping into a const data structure (e.g., const array of (ProjectType, &str field-name) or a build-time generated map), then implement get() as a single table lookup instead of a match.
- [x] `crates/swissarmyhammer-project-detection/src/types.rs:104` — ProjectType::marker_files() is a match statement that maps each ProjectType to its marker files array. Every new ProjectType requires manually updating this function. Extract the ProjectType-to-marker-files mapping into a const data structure (e.g., const array or lazy_static map), then implement marker_files() as a single lookup instead of a match.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/detect.rs:27` — project_type_name() is a match statement that maps each ProjectType to a human-readable display name. Every new ProjectType requires updating this function. Extract the ProjectType-to-display-name mapping into a const data structure, then implement project_type_name() as a single table lookup.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/detect.rs:29` — Three functions (project_type_name, project_type_key, partial_name_for_type) implement the identical match-on-ProjectType dispatch pattern with only the returned values differing; this creates maintenance burden—adding a ProjectType variant requires updating all three in lockstep instead of one place. Extract a single helper (struct ProjectTypeData { name, key, partial_name } returned from a helper that dispatches once) or use a macro to generate the shared match statement, so each function queries the result once instead of reimplementing the dispatch.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/detect.rs:43` — project_type_key() is a match statement that hardcodes serde key strings. These strings must match the serde(rename) attributes in types.rs, creating hidden coupling and duplication risk. Either: (1) derive the serde key programmatically using serde's serialization key extraction via a proc macro or trait, or (2) extract into a const data structure with a test that verifies keys match the serde attributes in types.rs.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/detect.rs:60` — partial_name_for_type() is a match statement that maps each ProjectType to a guideline partial template name (with Php returning None as a special case). Every new ProjectType requires updating this function. Extract the ProjectType-to-partial-name mapping into a const data structure (e.g., const array of (ProjectType, Option<&str>) tuples), then implement partial_name_for_type() as a single table lookup instead of a match.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/detect.rs:73` — Hardcoded default value 3 for max_depth configuration configures system behavior without explanation or named constant. Define a constant like `const DEFAULT_DETECT_MAX_DEPTH: usize = 3;` and use it here to clarify the default traversal depth.
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/detect.rs:97` — The `format_detected_projects` function has nesting depth of 4 levels (for → if let → if → if), which reaches the complexity threshold. The nested workspace info checks (if let ws, if is_root, if !members.is_empty) are sequential conditions that dilute readability. Extract workspace formatting into a helper function `format_workspace_info(ws: &WorkspaceInfo) -> String` to flatten the conditional chain. Replace the nested ifs with early returns or a dedicated function, reducing the nesting in the main loop to 2 levels (for → push_str calls only).
- [x] `crates/swissarmyhammer-tools/src/mcp/tools/code_context/detect.rs:161` — Function parameter accepts `Option<&String>` instead of the more general `Option<&str>`. Accepting the concrete String reference rather than the str primitive unnecessarily restricts callers and violates the principle to 'Accept generics, not concrete types.'. Change the parameter type to `Option<&str>` and update the call site to use `.as_deref()` instead of `.as_ref()`: `resolve_workspace_path(request.path.as_deref(), context)`.