---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kyyrz00d9tvzgksahsp6544a
  text: |-
    RED proven before any production change. Three new tests, all failing on the current code:

    ```
    FAIL swissarmyhammer-tools mcp::tools::ralph::state::tests::test_instruction_with_three_hyphen_line_round_trips
      assertion `left == right` failed
        left: "\"before\\n"
       right: "before\n---\nafter"

    FAIL swissarmyhammer-tools mcp::tools::ralph::state::tests::test_missing_closing_delimiter_returns_none
      assertion failed: parse_ralph_file(content).is_none()

    FAIL swissarmyhammer-tools health_registry::tests::test_prompt_health_checker_ignores_first_line_that_only_begins_with_hyphens
      notfm.md has no frontmatter, so it should report no YAML error, got: ["mapping values are not allowed in this context at line 2 column 6"]

    Summary 3 tests run: 0 passed, 3 failed
    ```

    The first failure is the card's defect on real data: `write_ralph` escapes a newline to a literal `\n`, so the instruction stays on one line, but `splitn(3, "---")` still cuts at the three-hyphen run inside the quoted scalar. The instruction comes back as `"before\n` -- truncated, with the opening quote left in place.

    The third failure shows the weak `starts_with("---")` gate is not merely permissive: a file whose first line is `---x` is read as frontmatter and `sah doctor` reports a YAML error against a file that has no frontmatter at all.
  timestamp: 2026-08-01T13:42:06.093513+00:00
- actor: claude-code
  id: 01kyyshkce59xkd68zrfbzbxac
  text: |-
    Dependency decision: promote the splitter to `swissarmyhammer-common`, do NOT add a tools -> entity edge.

    `swissarmyhammer-tools/Cargo.toml` does not list `swissarmyhammer-entity`. It reaches it only through `swissarmyhammer-kanban`. Adding the direct edge would compile (entity does not depend on tools, so no cycle), but it is the wrong placement: ARCHITECTURE.md puts `swissarmyhammer-entity` at Tier 3 "generic entity I/O" and reserves Tier 1 `swissarmyhammer-common` for "shared infrastructure consumed broadly across the workspace". A line-anchored text splitter is Tier 1 work, and pulling in the entity/store/watcher/undo stack to split a string is coupling with no payoff.

    `swissarmyhammer-common` already carries a `frontmatter` module and is already a direct dependency of BOTH crates, so the move added no dependency edge at all. No ARCHITECTURE.md change is needed: no new module, no new crate, no new edge direction.

    What moved: `crates/swissarmyhammer-entity/src/frontmatter.rs` was deleted whole and its function plus every one of its tests now live in `crates/swissarmyhammer-common/src/frontmatter.rs`, `pub` instead of `pub(crate)`. `swissarmyhammer-entity/src/io.rs:23` and `store.rs:23` now import it from common.

    Discovery worth recording: `swissarmyhammer-common::frontmatter::parse_frontmatter_internal` holds a FOURTH copy of this defect -- `content.splitn(3, "---\n")`, in the same file the splitter now lives in. It was left alone deliberately: it is a different function with a different contract, and its callers span swissarmyhammer-templating, mirdan, and every prompt/workflow load, which is a blast radius this card's verification scope could not cover. Filed as ^tv3692e with the two edge cases that move.
  timestamp: 2026-08-01T13:52:15.758582+00:00
- actor: claude-code
  id: 01kyysj0t5a7p22x0xyazvvk14
  text: |-
    GREEN. The same three tests, same command, after the fix:

    ```
    PASS swissarmyhammer-tools mcp::tools::ralph::state::tests::test_missing_closing_delimiter_returns_none
    PASS swissarmyhammer-tools mcp::tools::ralph::state::tests::test_instruction_with_three_hyphen_line_round_trips
    PASS swissarmyhammer-tools health_registry::tests::test_prompt_health_checker_ignores_first_line_that_only_begins_with_hyphens

    Summary 3 tests run: 3 passed, 1258 skipped
    ```

    Full scope, `cargo nextest run -p swissarmyhammer-tools -p swissarmyhammer-entity -p swissarmyhammer-common --no-fail-fast`: 2403 tests run, 2402 passed, 1 failed. The one failure is the known pre-existing flake `review_working_emits_progress_notifications_per_pair_when_token_supplied` (^pxhzk1s, ^t681xdv, ^yh4m6ed). It PASSED in the identical earlier run of the same tree and then FAILED alone on re-run in isolation, so it is nondeterministic and not a consequence of this change; it touches the review progress-notification path, no frontmatter code.

    `cargo clippy -p swissarmyhammer-tools --all-targets -- -D warnings` clean. `cargo fmt --check` clean.

    No existing test was edited. `git diff` on the two tools files is additive in the test modules only, and the common frontmatter diff has zero deletion lines. Two test additions to note honestly:
    - The moved `rejects_text_that_does_not_open_with_a_delimiter_line` gained one assertion (`"---x\ntitle: x\n---\n"` -> `None`), covering acceptance 3 at the unit level. An added case, not a changed one.
    - Added `test_text_without_a_frontmatter_block_returns_none` so `parse_ralph_file`'s docstring does not claim a None branch no fixture reaches.
  timestamp: 2026-08-01T13:52:29.509666+00:00
- actor: claude-code
  id: 01kyysj7fnh82547z975ee49vj
  text: |-
    ### implement — changed
    - evidence: 7 files — crates/swissarmyhammer-common/src/frontmatter.rs, crates/swissarmyhammer-entity/src/frontmatter.rs (deleted), crates/swissarmyhammer-entity/src/lib.rs, crates/swissarmyhammer-entity/src/io.rs, crates/swissarmyhammer-entity/src/store.rs, crates/swissarmyhammer-tools/src/health_registry.rs, crates/swissarmyhammer-tools/src/mcp/tools/ralph/state.rs. nextest 2402/2403 pass (1 known flake ^pxhzk1s), clippy clean, fmt clean.
    - next: /review
  timestamp: 2026-08-01T13:52:36.341835+00:00
- actor: claude-code
  id: 01kyyv1tbmw6rkv5svgh5hgep4
  text: |-
    ### review — findings
    - scope: `review sha HEAD~1..HEAD` (commit 3523b459)
    - evidence: engine reported 10 findings / 10 confirmed / 8 refuted. All 10 are out of scope: `git blame` gives every cited line to a different commit (ddb3c8da1, 4b8a48703, a3db85e01, 4227e1331, 569279fb5, 355650537). The cited lines also do not agree with the text of the findings — the same defect as ^j4d2613. The finding on `io.rs:104` is wrong on the facts: `write_entity` uses `if let Some(parent)` and cannot panic.
    - 1 real in-scope finding: `crates/swissarmyhammer-common/src/frontmatter.rs:6` and `:55` — the new module and function docstrings claim this is the workspace's only frontmatter splitter, but `parse_frontmatter_internal` at line 188 of the same file still uses `splitn(3, "---\n")`.
    - verified good: the cross-crate move is complete (no dangling reference, one definition at common:76); `line_content` and `is_delimiter_line` stay private; all 8 tests moved; the `starts_with` gate removal does not widen behavior; `parse_ralph_file` narrows exactly as the acceptance criteria ask, so the Stop hook is safe.
    - next: fix the two docstrings only. Do not touch `parse_frontmatter_internal` (^tv3692e owns it), and do not act on the 10 out-of-scope engine findings.
  timestamp: 2026-08-01T14:18:35.764435+00:00
- actor: claude-code
  id: 01kyyve2hzm36gkh1d42fwf10e
  text: |-
    Docs-only pass on the one in-scope finding. No production logic touched, no test edited.

    Three docstrings in `crates/swissarmyhammer-common/src/frontmatter.rs` changed. The first two are the finding. The third is a recurrence of the same cause found by the file-wide scan the review asked for.

    1. Module doc. Removed "This module provides common frontmatter parsing logic used by both workflow and prompt parsers" (false -- see the discovery below) and "[`split_frontmatter_body`] is the workspace's one frontmatter splitter ... the delimiter rule is stated once". Now names the four real callers, and says plainly it is not the only split, in this module or in the workspace.
    2. `split_frontmatter_body`. Removed "This is the workspace's single frontmatter splitter. Every reader ... calls it, so the delimiter rule cannot drift between them". Now: those four readers agree, [`parse_frontmatter`] in this module still cuts on the `---\n` substring until ^tv3692e closes it, other crates carry copies, call this one from a new reader.
    3. `parse_frontmatter`. Its doc said only "Handles content with YAML frontmatter delimited by `---` markers", which reads as full support for the format. Added the actual rule: this path splits on the `---\n` substring, so any `---` followed by a newline -- including one indented inside a YAML block scalar -- closes the block early and the content parses short. ^tv3692e named as the owner. Docstring only; `parse_frontmatter_internal` untouched.

    Verified before writing, not assumed:
    - `split_frontmatter_body` has exactly four production call sites: `entity/src/io.rs`, `entity/src/store.rs`, `parse_ralph_file` in `tools/src/mcp/tools/ralph/state.rs`, and `frontmatter_yaml_error` in `tools/src/health_registry.rs`.
    - The `splitn(3, "---\n")` claim: for `---\ntext: |-\n  before\n  ---\n  after\n---\nbody`, the indented `  ---\n` holds the substring `---\n`, so the frontmatter is cut at "before". The claim is exact, not hand-waved.

    Discoveries worth recording for ^tv3692e:
    - `swissarmyhammer_common::frontmatter::parse_frontmatter` and `parse_frontmatter_with_expansion` have ZERO callers anywhere in the workspace. Nothing imports them; `split_frontmatter_body` is the only item any other crate takes from this module. The old module doc claiming "used by both workflow and prompt parsers" was stale -- prompt parsing goes through `swissarmyhammer-templating`'s own copy. This may make ^tv3692e cheaper than its blast-radius note assumes.
    - The count is not four splitters, it is five. Besides the two in this module: `swissarmyhammer-templating::frontmatter::parse_frontmatter` (its own `find_closing_delimiter`), `mirdan::list::parse_frontmatter` (`strip_prefix("---")` then `find("---")` -- the substring defect, unguarded), and `swissarmyhammer-merge::frontmatter::split_frontmatter` (line-anchored, so correct). The mirdan one carries the same defect this card fixed and is not tracked anywhere yet.

    Left untouched, as instructed: the 10 engine findings marked out of scope, and `parse_frontmatter_internal` (^tv3692e owns it).
  timestamp: 2026-08-01T14:25:17.375640+00:00
- actor: claude-code
  id: 01kyyvfg4mc7vqg1ctaxss2z0b
  text: |-
    ### implement — changed
    - evidence: 1 file — crates/swissarmyhammer-common/src/frontmatter.rs (docstrings only, 26 insertions / 10 deletions, zero lines of code). `cargo nextest run -p swissarmyhammer-common -p swissarmyhammer-entity -p swissarmyhammer-tools --no-fail-fast`: 2403 tests run, 2403 passed, 0 skipped, 0 failed — the known flake ^pxhzk1s passed this run too. `cargo clippy -p swissarmyhammer-common --all-targets -- -D warnings` clean. `cargo fmt` clean. `cargo doc -p swissarmyhammer-common --no-deps` clean, so the three new intra-doc links resolve. No test edited.
    - follow-up card created: ^0zer2xf — `mirdan/src/list.rs` carries the same substring-split defect, untracked until now.
    - next: /review
  timestamp: 2026-08-01T14:26:04.052126+00:00
- actor: claude-code
  id: 01kyyw71evwmqnk8npht7evns4
  text: |
    ### review — clean

    Iteration 2. Scope: `review sha HEAD~1..HEAD` = commit `60a173bf2` only. Iteration 1
    (`3523b4594`) was not re-reviewed.

    - evidence: engine reported 5 findings, 0 of them in scope. No dated findings section
      appended; the card keeps its history unchanged.
    - next: card moves to `done`.

    #### The delta changes comment lines only

    `git show 60a173bf2 -- crates/` gives one code file,
    `crates/swissarmyhammer-common/src/frontmatter.rs`. Filtering the diff for added or
    removed lines that are not `//`, `//!` or `///` returns nothing. Zero executable lines
    changed. The other 8 files in the commit are `.kanban/tasks/*` board state, which is
    data.

    #### Every new factual claim is true

    1. **The four callers.** `split_frontmatter_body` has exactly four call sites in the
       workspace, and no fifth:
       - `crates/swissarmyhammer-entity/src/io.rs:354`
       - `crates/swissarmyhammer-entity/src/store.rs:175`
       - `crates/swissarmyhammer-tools/src/mcp/tools/ralph/state.rs:191`, inside
         `parse_ralph_file`
       - `crates/swissarmyhammer-tools/src/health_registry.rs:143`, inside
         `frontmatter_yaml_error`, which `PromptHealthChecker::run_health_checks` calls at
         line 101. "The prompt health check" names it correctly.

    2. **The second split.** `parse_frontmatter` (line 149) and
       `parse_frontmatter_with_expansion` (line 182) both call
       `parse_frontmatter_internal`, which gates on `content.starts_with("---\n")` and cuts
       with `content.splitn(3, "---\n")`. The two paths do disagree: an indented `  ---`
       line holds the `---\n` substring, so the substring path cuts there, while
       `is_delimiter_line` compares the whole line to `---` and keeps it in the frontmatter.

    3. **The `parse_frontmatter` doc.** "Any `---` immediately followed by a newline closes
       the block, even indented inside a YAML block scalar" matches `splitn(3, "---\n")`
       exactly. The substring match ignores what comes before the three hyphens, so an
       indented run cuts. The doc names the `---\n` substring explicitly, so it does not
       over-claim CRLF, which that path does not accept.

    4. **The other crates.** All three copies exist:
       - `crates/swissarmyhammer-templating/src/frontmatter.rs:35` — `starts_with("---")`
       - `crates/swissarmyhammer-merge/src/frontmatter.rs:44,47` —
         `lines.first() == Some(&"---")` and `position(|l| *l == "---")`, both whole-line
         comparisons on `str::lines()`. Line-anchored, so the comment on ^tv3692e calling it
         "line-anchored, so correct" is right.
       - `crates/mirdan/src/list.rs:406` — `strip_prefix("---")` then `find("---")`

       The module doc says only that the three crates "each carry a further copy of their
       own". That is true of all three, merge included, and it claims nothing about their
       correctness.

    No sentence swings from an over-claim to a different inaccuracy.

    #### The 5 engine findings are all out of scope. Do no work on them here.

    This is ^j4d2613 a third time. Not one cited line blames to `60a173bf2`, and not one
    resolves to the code the finding describes.

    | Cited | Blame | Code really at that line | Where the described code is |
    |---|---|---|---|
    | `common/frontmatter.rs:90` | `3523b4594` | `}` closing the `is_delimiter_line` guard | the `Frontmatter` derive is at 106 |
    | `common/frontmatter.rs:169` | `d6dd0ada4` | an empty `///` line | the `.unwrap()` is at 171 |
    | `common/frontmatter.rs:187` | `d6dd0ada4` | `}` closing `parse_frontmatter_with_expansion` | `starts_with("---\n")` is at 203 |
    | `common/frontmatter.rs:233` | `ddb3c8da1` | `});` | `parts.len() >= 3` is at 205 |
    | `common/frontmatter.rs:354`, `:355` | `ddb3c8da1` | `title: Test` and `---` inside a test fixture string | no `"---\n"` literal is at either line |

    Two of the five (`:187` CRLF handling, `:233` the literal `3`) name
    `parse_frontmatter_internal`, which ^tv3692e owns and this commit did not touch. A
    comment-only delta cannot cause a missing `PartialEq` derive, a CRLF gap, or a magic
    number.

    #### Prior findings left as they are

    The 10 findings from iteration 1 stay unchecked, as their own out-of-scope note on this
    card instructs. They are not this delta's work and do not block closure. The one
    in-scope finding from iteration 1 is checked, and this commit is the fix for it.
  timestamp: 2026-08-01T14:38:55.451008+00:00
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffffff8880
title: Two more copies of the frontmatter substring-split bug in swissarmyhammer-tools
---
The same defect fixed for the entity layer in ^fpcbeth survives in two more places, both in `swissarmyhammer-tools`. Each splits frontmatter on the bare three-hyphen **substring** instead of a line-anchored delimiter, so any occurrence inside the frontmatter block truncates the parse and silently drops every key after it.

## The two sites

`crates/swissarmyhammer-tools/src/health_registry.rs:101`

```rust
if content.starts_with("---") {
    let parts: Vec<&str> = content.splitn(3, "---").collect();
```

`crates/swissarmyhammer-tools/src/mcp/tools/ralph/state.rs:183`

```rust
fn parse_ralph_file(content: &str) -> Option<RalphState> {
    // Split on frontmatter delimiters
    let parts: Vec<&str> = content.splitn(3, "---").collect();
```

Found by the blast-radius sweep while implementing ^fpcbeth. Left out of that task deliberately: different crate, different files, so they were split rather than folded in.

## Why this is not merely cosmetic

^fpcbeth is not a theoretical bug. It fired three separate times on real data during the work that found it, each time silently blanking a card's `title` and dropping it off its board column, with the on-disk bytes intact so the damage only appeared on the next write. `parse_ralph_file` reads agent-authored state, and `health_registry` reads file content of the same shape. Both are exposed to the same free-form prose that triggered it.

Note `health_registry.rs:100` also gates on `content.starts_with("---")`, which accepts a file beginning `----` or `---x` as valid frontmatter.

## Required change

Use the shared splitter rather than re-deriving the parse a third time. ^fpcbeth added `split_frontmatter_body(content)` in `crates/swissarmyhammer-entity/src/frontmatter.rs`, which returns the frontmatter and body as borrowed slices, anchors both delimiters to whole lines, tolerates a trailing carriage return, and returns `None` for invalid frontmatter.

If `swissarmyhammer-tools` should not depend on `swissarmyhammer-entity` for this, promote the splitter to a crate both can use — but do not write a third copy of the logic.

## Acceptance

- Neither file constructs its own frontmatter split; both go through one shared implementation.
- A ralph state file whose instruction text contains a bare three-hyphen line on its own line round-trips with every field intact. Prove it RED first.
- The `starts_with` gate no longer accepts a first line that merely begins with three hyphens.
- A malformed file with no closing delimiter is still rejected, not treated as all-frontmatter.
- No behavior change for well-formed input: existing tests pass unedited.

Blocked by nothing, but do it after ^fpcbeth lands so the shared splitter exists to call. #bug

## Review Findings (2026-08-01 09:00)

- [ ] `crates/swissarmyhammer-common/src/frontmatter.rs:118` — The Frontmatter construction for the no-metadata case is verbatim duplicated at line 159. Both return identical `Frontmatter { metadata: None, content: content.to_string() }`. If the default Frontmatter structure ever changes, both locations must be updated in lockstep or drift occurs. Extract a helper function like `fn no_metadata_frontmatter(content: &str) -> Result<Frontmatter>` that returns `Ok(Frontmatter { metadata: None, content: content.to_string() })`, then call it in both places to eliminate the duplication.
- [ ] `crates/swissarmyhammer-common/src/frontmatter.rs:127` — Docstring example uses `.unwrap()` instead of `?`, teaching defensive rather than idiomatic error handling. Examples should show proper error propagation. Wrap the example in a function that returns `Result`: `# fn main() -> Result<(), Box<dyn std::error::Error>> { ... let result = parse_frontmatter_with_expansion(content, &expander)?; ... # Ok(()) # }` to allow proper `?` usage.
- [ ] `crates/swissarmyhammer-entity/src/io.rs:104` — Public function `write_entity` panics on invalid input (paths with no parent directory) but this panic condition is not documented in the docstring. Per the documentation rule, 'Panics, errors, and safety requirements documented.'. Add a 'Panics' section to the docstring: `///\n/// # Panics\n/// Panics if `path` has no parent directory (e.g., a root path like `/`).
- [ ] `crates/swissarmyhammer-entity/src/io.rs:250` — Public function `trash_entity_files` panics on invalid input (paths with no filename component) but this panic condition is not documented in the docstring. Per the documentation rule, 'Panics, errors, and safety requirements documented.'. Add a 'Panics' section to the docstring: `///\n/// # Panics\n/// Panics if `path` has no filename component (e.g., a directory path ending in `/`).
- [ ] `crates/swissarmyhammer-entity/src/io.rs:278` — Public function `restore_entity_files` panics on invalid input (paths with no filename component) but this panic condition is not documented in the docstring. Per the documentation rule, 'Panics, errors, and safety requirements documented.'. Add a 'Panics' section to the docstring: `///\n/// # Panics\n/// Panics if `path` has no filename component (e.g., a directory path ending in `/`).
- [ ] `crates/swissarmyhammer-tools/src/health_registry.rs:22` — The directory existence check and reporting pattern is near-verbatim duplicated at line 31. Both follow: if exists, count files and report 'Found'; else report 'Not found (optional)'. If the reporting logic or structure ever changes, both locations must be updated in lockstep or drift occurs. Extract a helper function like `fn check_prompts_dir(label: &str, path: &Path, cat: &str) -> Vec<HealthCheck>` that performs the check and returns the appropriate HealthCheck objects, then call it for both user and local directories to eliminate the duplicated check-and-report logic.
- [ ] `crates/swissarmyhammer-tools/src/health_registry.rs:46` — The markdown file filter chain is near-verbatim duplicated from `count_markdown_files`. Both use identical `walkdir` setup with the same four filters to select `.md` files. If filter criteria ever change (e.g., to include `.markdown`), both locations must be updated in lockstep or drift occurs. Extract a shared helper function like `fn iter_markdown_files(path: &Path) -> impl Iterator<Item = DirEntry>` that returns the filtered iterator, then use it in both `count_markdown_files` (call `.count()` on it) and in the loop at line 46 (iterate directly). This eliminates the duplicated filter logic.
- [ ] `crates/swissarmyhammer-tools/src/health_registry.rs:68` — Excessive nesting in YAML error checking loop: 5 levels deep (for → for → match → Ok arm → if let), making the control flow hard to follow and reason about. Extract the file-reading and error-checking logic into a helper function, e.g. `collect_yaml_errors_in_dir(dir: &Path) -> Vec<(PathBuf, String)>`, which returns the error list directly. Then replace the nested loop with a simple call to that helper.
- [ ] `crates/swissarmyhammer-tools/src/health_registry.rs:88` — The library initialization pattern is near-verbatim duplicated at line 102. Both follow: `Arc::new(RwLock::new(Library::new()))` → get write lock → call `load_defaults()`. If initialization logic ever changes (e.g., different load method, error handling), both locations must be updated in lockstep or drift occurs. Extract a generic helper function like `fn init_library<T: NewAndDefaults>(name: &str) -> Arc<RwLock<T>>` or use a closure to parameterize the library type, then call it for both skill and agent libraries to eliminate the duplicated initialization pattern.
- [ ] `crates/swissarmyhammer-tools/src/mcp/tools/ralph/state.rs:141` — The u32 field parsing pattern is near-verbatim duplicated at line 146. Both blocks follow identical structure: `strip_prefix(KEY)`, `trim().parse::<u32>()`, assign to variable. If the parsing logic ever changes (e.g., to handle negative numbers, validate ranges), both locations must be updated in lockstep or drift occurs. Extract a helper function like `fn parse_u32_field(line: &str, key: &str) -> Option<u32>` that returns the parsed value if the key matches, then use it for both fields: `if let Some(n) = parse_u32_field(line, "iteration") { iteration = n; } else if let Some(n) = parse_u32_field(line, "max_iterations") { max_iterations = n; }`.

## Review Verification (2026-08-01 09:00)

The scope of this review is the delta of commit 3523b459 (`HEAD~1..HEAD`).

### The 10 engine findings above are out of scope. Do no work on them here.

`git blame` gives every cited line to a different commit. Not one of the 10
lines was changed by 3523b459:

| Cited | Blame commit |
|---|---|
| `common/src/frontmatter.rs:118`, `:127` | `ddb3c8da1` |
| `entity/src/io.rs:104` | `4b8a48703` |
| `entity/src/io.rs:250` | `a3db85e01` |
| `entity/src/io.rs:278` | `4227e1331` |
| `tools/src/health_registry.rs:22`, `:46`, `:68`, `:88` | `569279fb5` |
| `tools/.../ralph/state.rs:141` | `355650537` |

### The cited lines also do not agree with the text of the findings

This is the same defect as ^j4d2613. The line numbers point at other code:

| Cited | Code that is really at that line | Where the described code is |
|---|---|---|
| `common/frontmatter.rs:118` | doc comment `/// use ...::parse_frontmatter;` | `metadata: None` is at 181 and 223 |
| `common/frontmatter.rs:127` | doc comment `/// "#;` | the `.unwrap()` named is at 164 |
| `io.rs:104` | `let content = if let Some(ref body_field)` | see the note below |
| `io.rs:250` | inside `trash_entity_files` | the `.expect` is at 246 |
| `io.rs:278` | `return Err(EntityError::RestoreFromTrashFailed ...)` | the `.expect` is at 273 |
| `health_registry.rs:22` | a doc comment | the two directory checks are at 46-62 and 65-79 |
| `health_registry.rs:46` | `if let Some(home) = dirs::home_dir()` | the walkdir filter chain is at 93-97 |
| `health_registry.rs:68` | `checks.push(HealthCheck::ok(` | the nested loop is at 88-113 |
| `health_registry.rs:88` | `for dir in dirs_to_check {` | the `Arc::new(RwLock::new(...))` set is at 194-206 |
| `ralph/state.rs:141` | `Ok(parse_ralph_file(&content))` | the u32 parse pair is at 200-207 |

The finding on `io.rs:104` is also wrong on the facts. `write_entity` cannot
panic on a path with no parent. It uses `if let Some(parent) = path.parent()`
and does nothing when the path has no parent.

### What the delta itself does correctly

- The cross-crate move is complete. No reference to the deleted
  `swissarmyhammer-entity` module is left. Exactly one `split_frontmatter_body`
  exists in the workspace, at `common/src/frontmatter.rs:76`.
- The visibility is correct. `line_content` (line 42) and `is_delimiter_line`
  (line 49) stay private. Only `split_frontmatter_body` is `pub`.
- All 8 tests moved from the deleted module into `common`.
- Removing the `starts_with("---")` gate does not widen the behavior.
  `frontmatter_yaml_error` uses `?`, so a `None` from the splitter pushes no
  diagnostic. A file that has real frontmatter is still parsed and still
  reports a YAML error.
- `parse_ralph_file` narrows correctly. The old `parts.len() < 2` test let
  through any text that held a `---` substring anywhere. The new code needs a
  first line of exactly `---` and a closing delimiter line. The acceptance
  criteria of this card ask for that narrowing, and two new tests hold it. A
  file that `write_ralph` writes always has both delimiter lines at column 0
  and escapes the newlines in the instruction, so it round-trips. The Stop
  hook is not at risk.

### One real finding, in scope

- [x] `crates/swissarmyhammer-common/src/frontmatter.rs:6` and `:55` — Two new
  docstrings claim this splitter is the only one, and the same file disagrees.
  The module doc says "[`split_frontmatter_body`] is the workspace's one
  frontmatter splitter" and "the delimiter rule is stated once". The function
  doc says "This is the workspace's single frontmatter splitter. Every reader
  of the frontmatter + markdown body format calls it, so the delimiter rule
  cannot drift between them". `parse_frontmatter_internal`, at line 188 of this
  same file, still splits with `splitn(3, "---\n")`, and the public
  `parse_frontmatter` and `parse_frontmatter_with_expansion` both reach it.
  Thus "one splitter", "single splitter", "every reader calls it" and "cannot
  drift" are all false as written. Correct the two docstrings to state what is
  true now: this is the line-anchored splitter for the readers of the
  frontmatter + markdown body format, and a second splitter for the
  YAML-parsing path stays in this file until ^tv3692e closes it. Change only
  the docstrings. Do not change `parse_frontmatter_internal` — ^tv3692e owns
  that code.