---
depends_on:
- 01KKMKCX7JMCC9MW9FHE593V1J
position_column: done
position_ordinal: '8580'
title: Implement fuzzy search over Entity fields
---
## What
Create `swissarmyhammer-entity-search/src/fuzzy.rs` — fuzzy search using `fuzzy-matcher` SkimMatcherV2 over Entity objects. Iterates entity.fields values (stringified) and matches against the query.

Pattern to follow: `swissarmyhammer-code-context/src/ops/search_symbol.rs:83-104`

Implementation:
- `fuzzy_search(entities: &[Entity], query: &str, limit: usize) -> Vec<SearchResult>`
- For each entity, run `matcher.fuzzy_match()` against each field value (strings, stringified arrays) and body
- Track which field produced the best score via `matched_field`
- Normalize scores to [0.0, 1.0] range
- Return top-k by descending score

## Acceptance Criteria
- [ ] Fuzzy search finds entities by partial field match (title, name, etc.)
- [ ] Fuzzy search finds entities by body/description content
- [ ] Results include `matched_field` indicating which field matched best
- [ ] Results sorted by descending score, limited to `limit`
- [ ] Works for any entity type (task, tag, column, etc.)

## Tests
- [ ] `test_fuzzy_title_match` — \"auth\" matches entity with title \"Authentication flow\"
- [ ] `test_fuzzy_field_match` — matches on arbitrary field values
- [ ] `test_fuzzy_body_match` — matches on body field content
- [ ] `test_fuzzy_ranking` — better matches score higher
- [ ] `test_fuzzy_no_match` — returns empty for non-matching query
- [ ] `cargo nextest run -p swissarmyhammer-entity-search fuzzy`