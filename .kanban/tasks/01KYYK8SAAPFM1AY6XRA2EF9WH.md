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
position_column: doing
position_ordinal: '8480'
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