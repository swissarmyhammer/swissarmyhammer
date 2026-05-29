---
assignees:
- claude-code
position_column: done
position_ordinal: ffffffffffffffffffffffffffffffffffffb680
title: Remove unused web content-fetcher tag extraction (extract_tags + ContentMetadata.tags)
---
## What
`swissarmyhammer-web`'s content fetcher has a vestigial "tags" concept that nothing consumes. `ContentFetcher::extract_tags` (`crates/swissarmyhammer-web/src/search/content_fetcher.rs:777-823`) substring-matches a hardcoded list of ~19 tech keywords and regex-greps `#hashtags`, stuffing up to 10 into `ContentMetadata.tags` (`crates/swissarmyhammer-web/src/types.rs:332-334`). Repo-wide, `metadata.tags` is written exactly once (`content_fetcher.rs:683`) and read only in this module's own unit tests — no ranking, filtering, dedup, scoring, or synthesis path consumes it. It just inflates the serialized search-result JSON with low-signal keyword soup, and its `#hashtag` regex is the incidental "second tag parser" we flagged.

Delete it. This leaves `swissarmyhammer-kanban/src/tag_parser.rs` as the single tag parser.

Remove:
- `fn extract_tags` (`content_fetcher.rs:777-823`).
- The call site + field init: `let tags = self.extract_tags(content);` (line 674) and `tags,` in the `ContentMetadata { … }` literal (line 683) in `extract_metadata`.
- The `tags` field from `ContentMetadata` (`types.rs:332-334`), including its `#[serde(default)]`.
- The tag-specific unit tests: `test_extract_tags_tech_keywords`, `test_extract_tags_hashtags`, `test_extract_tags_no_short_hashtags`, `test_extract_tags_no_duplicate_hashtag_and_keyword`, `test_extract_tags_truncates_at_ten`, `test_extract_tags_empty_content` (content_fetcher.rs ~1246-1301), and the `metadata.tags.contains(...)` assertions in `test_extract_metadata_tags_populated` (~1639-1656) — remove that test or strip its tags assertions.
- If `use ... Regex` (and the `regex` dep usage) becomes unused after removal, drop the now-dead import; if `Regex` is still used elsewhere in the file, leave it.

Note: removing a `#[serde(default)]` field is backward-compatible on deserialize (older JSON carrying `tags` is ignored). `ContentMetadata` is serialized into `SearchResultContent` (`types.rs:270`) handed to the model — removing the field simply slims that output; confirm no other crate reads `.tags` off this struct (grep already shows none).

Out of scope: real content keyword/topic extraction wired into ranking is a separate feature — file its own card if wanted; do not build it here.

## Acceptance Criteria
- [ ] `extract_tags` removed; no call site remains in `extract_metadata`
- [ ] `ContentMetadata.tags` field removed from `types.rs`
- [ ] All `test_extract_tags_*` tests removed; `test_extract_metadata_tags_populated` removed or stripped of tags assertions
- [ ] Repo-wide grep for `extract_tags` and `metadata.tags` returns no production references (tests included)
- [ ] No unused-import / dead-code warnings introduced (drop `Regex` import only if it becomes unused)
- [ ] `cargo clippy -p swissarmyhammer-web --all-targets` clean

## Tests
- [ ] `cargo nextest run -p swissarmyhammer-web` passes (the existing content-fetcher suite still green after the tag tests are removed)
- [ ] `cargo build -p swissarmyhammer-web` succeeds with no warnings

## Workflow
- Pure deletion (no new behavior), so not `/tdd`: make the removal, then prove no regression via the existing `swissarmyhammer-web` suite + a grep gate confirming `extract_tags`/`ContentMetadata.tags` are gone.