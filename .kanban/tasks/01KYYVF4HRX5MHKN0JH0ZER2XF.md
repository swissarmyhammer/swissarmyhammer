---
assignees:
- claude-code
comments:
- actor: claude-code
  id: 01kzebkeattkdx9skfpvtzy2y5
  text: |-
    Research done.

    Dependency edge is already there: `crates/mirdan/Cargo.toml` line 25 lists `swissarmyhammer-common = { workspace = true }`. No Cargo change needed. `swissarmyhammer_common::frontmatter` is a `pub mod` and `split_frontmatter_body` is `pub`.

    Prevailing call pattern (five sites): read the file, call `split_frontmatter_body(&content)?`, feed the frontmatter slice straight to `serde_yaml_ng::from_str`. No trim of the slice. `entity/io.rs`, `entity/store.rs`, `tools/health_registry.rs`, `config/model.rs`, `tools/ralph/state.rs` all do this. list.rs follows the same shape.

    Dropping the `content.trim()` in list.rs is part of the fix, not a side effect: `trim()` lets `"  ---\n..."` open a frontmatter block, which is exactly the loose delimiter rule the card removes.

    Three RED cases confirmed against the old code by hand:
    1. `description: Uses --- as a separator` — old `find("---")` cuts at the run inside the value, so the parsed map holds only `name` and a truncated `description: Uses`. `metadata.version` is lost and `read_frontmatter_version` falls back to "latest".
    2. Opening line `---description: leaked` — old `strip_prefix("---")` accepts it, and the remainder parses as valid YAML, so the reader returns keys from a file that has no frontmatter block.
    3. No closing delimiter line, but a `---` run inside a value — old code finds that run and returns a partial map instead of `None`.

    An opening line of exactly `----` is `None` under both old and new code (the old code's leftover `-` makes the YAML unparseable), so that example is a pin, not a RED case. It is still in the test because the acceptance list names it.

    Discovered out of scope: four further copies of the same substring split live in the same crate — `mirdan/src/applier.rs` (`install/applier.rs`), `mirdan/src/mcp_config.rs`, `mirdan/src/info.rs`, `mirdan/src/git_source.rs`. The card scopes this work to `list.rs`, so they get their own card.
  timestamp: 2026-08-07T14:56:26.970402+00:00
- actor: claude-code
  id: 01kzec1kxdeq83pds0fn9941xb
  text: |-
    Implementation landed. RED first, then GREEN.

    RED run (`cargo nextest run -p mirdan --lib list::`): 16 tests, 13 passed, 3 failed. Each failed for the predicted reason:
    - `three_hyphen_run_in_a_description_keeps_every_frontmatter_key` — description read back as "Uses", not "Uses --- as a separator".
    - `an_opening_line_with_text_after_the_hyphens_is_not_a_delimiter` — name read back as Some("test-skill"), not None.
    - `a_file_with_no_closing_delimiter_line_reads_no_frontmatter` — name read back as Some("test-skill"), not None.

    `an_opening_line_of_more_than_three_hyphens_is_not_a_delimiter` passed on the old code, as the research comment predicted. It is a pin for the `----` example the acceptance list names.

    GREEN run: 16 of 16 pass. Every pre-existing test is unedited.

    `parse_frontmatter` now reads the file and calls `split_frontmatter_body(&content)?`. The `content.trim()` is gone: `trim()` let a leading-whitespace line open a frontmatter block, which the line-anchored rule forbids.

    Two further things the rules required, both inside the blast radius of this change:

    1. `cargo clippy -W missing_docs` reported five undocumented public fields on `InstalledPackage` in the file I edited (`name`, `description`, `package_type`, `version`, `targets`). The missing-docs rule is file-wide, not diff-wide, so I documented all five. `list.rs` now reports zero `missing_docs`.

    2. The module documentation of `swissarmyhammer-common/src/frontmatter.rs` enumerated the readers of `split_frontmatter_body` and counted them ("Five readers ... All five therefore agree"). My change made that count wrong. It was already wrong by one before me — commit a193b9427 added the two `swissarmyhammer-config` model-config readers and never updated the count. I named every reader and removed the hand-maintained number from the function docstring, which now points at the module list instead of repeating it. The "other crates carry copies" sentence now says "among them", because templating and merge are not the only ones — skills, agents, validators, sem, code_context, and markdowndown carry copies too.

    Verification, all from the repo root:
    - `cargo nextest run -p mirdan -p mirdan-app -p swissarmyhammer-common` — 1062 passed, 0 failed, 0 skipped.
    - `cargo clippy -p mirdan -p mirdan-app -p swissarmyhammer-common --all-targets --all-features -- -D warnings` — clean.
    - `cargo check --workspace --all-targets` — zero warnings, zero errors.
    - `cargo doc -p mirdan -p swissarmyhammer-common --no-deps` — the warnings it prints are pre-existing and unrelated (`create_temp_dir` in `fs_utils`, unresolved links in mirdan module docs). No link I wrote is among them.
    - `cargo fmt` run on both crates.

    Blast radius: `read_frontmatter_name` is the only public reader. One caller lives outside the crate, `apps/mirdan-app/src/commands.rs`, which matches a skill directory by frontmatter name. The signature does not change. The behavior is stricter only for malformed frontmatter, and it now reads the name correctly for a SKILL.md whose description holds a three-hyphen run, where the truncated YAML used to fail to parse and lose the name outright. mirdan-app tests pass.

    Filed `^nk1yr64` for the four remaining copies of this defect in the same crate: `install/applier.rs`, `mcp_config.rs`, `info.rs`, `git_source.rs`. `applier.rs` is the worst — it `.expect("SKILL.md must open with frontmatter")`, so a bad read panics.

    Unrelated observation, not touched: a stray `.kanban` directory sits at `crates/swissarmyhammer-mcp-proxy/.kanban`. It dates to Jul 31 and nothing in it is newer than this session, so this work did not create it.
  timestamp: 2026-08-07T15:04:11.437938+00:00
- actor: claude-code
  id: 01kzec1vkw20cgkwz8bxd5w597
  text: |-
    ### implement — changed
    - evidence: 2 files — crates/mirdan/src/list.rs, crates/swissarmyhammer-common/src/frontmatter.rs. 4 new tests (3 RED first, 1 pin). `cargo nextest run -p mirdan -p mirdan-app -p swissarmyhammer-common`: 1062 passed, 0 failed, 0 skipped. `cargo clippy --all-targets --all-features -- -D warnings`: clean. `cargo check --workspace --all-targets`: zero warnings. Follow-up card `^nk1yr64`.
    - next: /review
  timestamp: 2026-08-07T15:04:19.324828+00:00
position_column: doing
position_ordinal: '8380'
title: mirdan list.rs carries the frontmatter substring-split defect
---
`crates/mirdan/src/list.rs` has a private `parse_frontmatter` with the same defect ^fpcbeth and ^a2ef9wh fixed elsewhere.

```rust
fn parse_frontmatter(path: &Path) -> Option<serde_yaml_ng::Value> {
    let content = std::fs::read_to_string(path).ok()?;
    let content = content.trim();
    let rest = content.strip_prefix("---")?;
    let end = rest.find("---")?;
    let frontmatter = &rest[..end];
    serde_yaml_ng::from_str(frontmatter).ok()
}
```

Two faults, both already fixed in the other copies:

1. `rest.find("---")` finds the three-hyphen **substring** anywhere, not a delimiter line. A `---` inside a description, or a horizontal rule indented in a block scalar, cuts the frontmatter short. The truncated text usually still parses as valid YAML, so the read silently returns a partial mapping instead of failing.
2. `strip_prefix("---")` accepts a first line of `----` or `---x` as an opening delimiter.

## Who reads it

`read_frontmatter_name`, `read_frontmatter_description`, and `read_frontmatter_version` in the same file. They read `SKILL.md`, `VALIDATOR.md`, and `TOOL.md` to build the `mirdan list` output. A skill whose description holds a three-hyphen run can lose its `name` or its `metadata.version` and list wrong.

## Required change

Call `swissarmyhammer_common::frontmatter::split_frontmatter_body`, which is line-anchored, tolerates CRLF, and returns `None` for a malformed block. Do not write a sixth copy.

Check the dependency edge first: confirm `mirdan/Cargo.toml` already lists `swissarmyhammer-common`, and add it if not. ^a2ef9wh chose `swissarmyhammer-common` as the Tier 1 home for this splitter for exactly this reason.

## Acceptance

- `list.rs` builds no frontmatter split of its own.
- RED first: a `SKILL.md` whose `description` holds a three-hyphen run keeps every frontmatter key, `name` and `metadata.version` included.
- A first line of `----` or `---x` is not read as an opening delimiter.
- A file with no closing delimiter line still returns `None`.
- No behavior change for well-formed input: existing tests pass unedited.

Found by the file-wide over-claim scan on ^a2ef9wh. Related: ^tv3692e (the `parse_frontmatter` copy in `swissarmyhammer-common`). #bug